//! Order-0 rANS entropy coder for the DCT coefficient streams.
//!
//! Encodes a byte stream with a static frequency table stored in the payload.
//! rANS with 32-bit state, 12-bit probability precision, byte-wise renormalisation.

const SCALE_BITS: u32 = 12;
const SCALE: u32 = 1 << SCALE_BITS;
const RANS_L: u32 = 1 << 23;

pub struct Table {
    pub freq: [u32; 256],
    pub cum: [u32; 256],
}

fn build_table(data: &[u8]) -> Table {
    let mut counts = [0u32; 256];
    for byte in data {
        counts[*byte as usize] += 1;
    }
    let mut freq = [0u32; 256];
    let mut used = 0u32;
    for count in counts {
        if count > 0 {
            used += 1;
        }
    }
    if used == 0 {
        freq[0] = SCALE;
    } else {
        let total: u32 = counts.iter().sum();
        let mut assigned = 0u32;
        for symbol in 0..256usize {
            if counts[symbol] == 0 {
                continue;
            }
            let mut scaled = ((counts[symbol] as u64 * SCALE as u64) / total as u64) as u32;
            if scaled == 0 {
                scaled = 1;
            }
            freq[symbol] = scaled;
            assigned += scaled;
        }
        let mut symbol = 0usize;
        while assigned < SCALE {
            while counts[symbol] == 0 {
                symbol = (symbol + 1) % 256;
            }
            freq[symbol] += 1;
            assigned += 1;
            symbol = (symbol + 1) % 256;
        }
        while assigned > SCALE {
            let mut moved = false;
            for value in freq.iter_mut() {
                if *value > 1 && assigned > SCALE {
                    *value -= 1;
                    assigned -= 1;
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
    }
    let mut cum = [0u32; 256];
    let mut running = 0u32;
    for symbol in 0..256usize {
        cum[symbol] = running;
        running += freq[symbol];
    }
    Table { freq, cum }
}

/// Encodes bytes.
/// Layout: u16 table_size, table (symbol u8, freq u16), u32 raw_len,
/// u32 stream_len, then state (u32 LE) followed by renorm bytes.
pub fn encode(data: &[u8]) -> Vec<u8> {
    let table = build_table(data);
    let mut symbols: Vec<u8> = (0..256usize)
        .filter(|symbol| table.freq[*symbol] > 0)
        .map(|symbol| symbol as u8)
        .collect();
    symbols.sort_unstable();
    let mut out = Vec::with_capacity(data.len() + 64);
    out.extend_from_slice(&(symbols.len() as u16).to_le_bytes());
    for symbol in &symbols {
        out.push(*symbol);
        out.extend_from_slice(&(table.freq[*symbol as usize] as u16).to_le_bytes());
    }
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    let mut state: u32 = RANS_L;
    let mut renorm: Vec<u8> = Vec::with_capacity(data.len() + 16);
    for byte in data.iter().rev() {
        let symbol = *byte as usize;
        let freq = table.freq[symbol];
        let cum = table.cum[symbol];
        let max_state = ((RANS_L >> SCALE_BITS) << 8) * freq;
        while state >= max_state {
            renorm.push(state as u8);
            state >>= 8;
        }
        state = ((state / freq) << SCALE_BITS) + (state % freq) + cum;
    }
    renorm.reverse();
    out.extend_from_slice(&(renorm.len() as u32).to_le_bytes());
    out.extend_from_slice(&state.to_le_bytes());
    out.extend_from_slice(&renorm);
    out
}

pub fn decode(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 2 {
        return Err("rANS payload too short".into());
    }
    let table_size = u16::from_le_bytes([data[0], data[1]]) as usize;
    let mut cursor = 2usize;
    let mut freq = [0u32; 256];
    for _ in 0..table_size {
        if cursor + 3 > data.len() {
            return Err("truncated rANS table".into());
        }
        let symbol = data[cursor] as usize;
        freq[symbol] = u16::from_le_bytes([data[cursor + 1], data[cursor + 2]]) as u32;
        cursor += 3;
    }
    let mut cum = [0u32; 256];
    let mut running = 0u32;
    for symbol in 0..256usize {
        cum[symbol] = running;
        running += freq[symbol];
    }
    if running != SCALE {
        return Err(format!("rANS table sums to {running}, expected {SCALE}"));
    }
    if cursor + 8 > data.len() {
        return Err("truncated rANS header".into());
    }
    let raw_len =
        u32::from_le_bytes([data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3]]) as usize;
    cursor += 4;
    let stream_len =
        u32::from_le_bytes([data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3]]) as usize;
    cursor += 4;
    if cursor + 4 + stream_len > data.len() {
        return Err("rANS stream outside payload".into());
    }
    let mut state = u32::from_le_bytes([data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3]]);
    cursor += 4;
    let renorm = &data[cursor..cursor + stream_len];
    let mut position = 0usize;
    let mut out: Vec<u8> = Vec::with_capacity(raw_len);
    let mut lookup = [0usize; SCALE as usize];
    for symbol in 0..256usize {
        for slot in cum[symbol]..cum[symbol] + freq[symbol] {
            lookup[slot as usize] = symbol;
        }
    }
    for _ in 0..raw_len {
        let slot = state & (SCALE - 1);
        let symbol = lookup[slot as usize];
        out.push(symbol as u8);
        state = freq[symbol] * (state >> SCALE_BITS) + slot - cum[symbol];
        while state < RANS_L {
            let byte = *renorm
                .get(position)
                .ok_or_else(|| "rANS renorm underflow".to_string())?;
            position += 1;
            state = (state << 8) | u32::from(byte);
        }
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    #[test]
    fn round_trip() {
        let cases: Vec<Vec<u8>> = vec![
            Vec::new(),
            vec![0],
            vec![7; 1000],
            (0..5000).map(|i| (i % 251) as u8).collect(),
            (0..3000).map(|i| if i % 7 == 0 { 200 } else { 3 }).collect(),
        ];
        for case in cases {
            let encoded = super::encode(&case);
            let decoded = super::decode(&encoded).expect("decode");
            assert_eq!(case, decoded);
        }
    }
}
