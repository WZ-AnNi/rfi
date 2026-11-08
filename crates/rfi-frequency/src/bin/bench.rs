//! Measures encode/decode throughput and the transform/entropy split.

use std::time::Instant;

fn zstd_roundtrip(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let compressed = zstd::bulk::compress(data, 19)?;
    Ok(zstd::bulk::decompress(&compressed, data.len())?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let input = args.get(1).ok_or("input image required")?;
    let image = image::open(input)?.to_rgba8();
    let step: f32 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(40.0);
    let rounds: usize = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(5);

    let warm = rfi_frequency::encode(&image, step)?;
    let _ = rfi_frequency::decode(&warm)?;

    let mut encode_ns = 0u128;
    let mut bytes = warm;
    for _ in 0..rounds {
        let start = Instant::now();
        bytes = rfi_frequency::encode(&image, step)?;
        encode_ns += start.elapsed().as_nanos();
    }

    let mut decode_ns = 0u128;
    for _ in 0..rounds {
        let start = Instant::now();
        let decoded = rfi_frequency::decode(&bytes)?;
        decode_ns += start.elapsed().as_nanos();
        std::hint::black_box(&decoded);
    }

    let mut transform_ns = 0u128;
    let mut coeff = Vec::new();
    for _ in 0..rounds {
        let start = Instant::now();
        coeff = rfi_frequency::bench_transform_only(&image, step)?;
        transform_ns += start.elapsed().as_nanos();
    }
    let mut entropy_ns = 0u128;
    for _ in 0..rounds {
        let start = Instant::now();
        let back = zstd_roundtrip(&coeff)?;
        entropy_ns += start.elapsed().as_nanos();
        std::hint::black_box(&back);
    }

    let pixels = f64::from(image.width()) * f64::from(image.height());
    let per = rounds as f64;
    let encode_s = encode_ns as f64 / 1e9 / per;
    let decode_s = decode_ns as f64 / 1e9 / per;
    let transform_s = transform_ns as f64 / 1e9 / per;
    let entropy_s = entropy_ns as f64 / 1e9 / per;
    println!("image      {}x{}", image.width(), image.height());
    println!("bytes      {}", bytes.len());
    println!(
        "encode     {:.1} ms  ({:.1} MP/s)",
        encode_s * 1e3,
        pixels / encode_s / 1e6
    );
    println!(
        "decode     {:.1} ms  ({:.1} MP/s)",
        decode_s * 1e3,
        pixels / decode_s / 1e6
    );
    println!("  transform  {:.1} ms", transform_s * 1e3);
    println!("  entropy    {:.1} ms", entropy_s * 1e3);
    println!();
    println!("raw stream sizes:");
    for (name, size) in rfi_frequency::bench_stream_sizes(&image, step)? {
        println!("  {name:<10} {size:>9}");
    }
    Ok(())
}
