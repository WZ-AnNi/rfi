use std::path::{Path, PathBuf};

use image::RgbaImage;
use rfi_frequency::{decode, encode};

fn psnr(a: &RgbaImage, b: &RgbaImage) -> f64 {
    let mut sum = 0.0f64;
    for (left, right) in a.pixels().zip(b.pixels()) {
        for c in 0..4 {
            let diff = f64::from(left.0[c]) - f64::from(right.0[c]);
            sum += diff * diff;
        }
    }
    let mse = sum / (a.width() as f64 * a.height() as f64 * 4.0);
    if mse <= 0.0 {
        f64::INFINITY
    } else {
        10.0 * (255.0f64 * 255.0 / mse).log10()
    }
}

fn load(path: &Path) -> Result<RgbaImage, Box<dyn std::error::Error>> {
    Ok(image::open(path)?.to_rgba8())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: rfi-frequency encode|decode|rd ...");
        return Ok(());
    }
    match args[1].as_str() {
        "encode" => {
            let input = PathBuf::from(args.get(2).ok_or("input")?);
            let output = PathBuf::from(args.get(3).ok_or("output")?);
            let step: f32 = args.get(4).and_then(|value| value.parse().ok()).unwrap_or(20.0);
            let image = load(&input)?;
            let bytes = encode(&image, step)?;
            std::fs::write(&output, &bytes)?;
            println!("{} bytes, step {step}", bytes.len());
        }
        "decode" => {
            let input = PathBuf::from(args.get(2).ok_or("input")?);
            let output = PathBuf::from(args.get(3).ok_or("output")?);
            let bytes = std::fs::read(&input)?;
            let image = decode(&bytes)?;
            image.save(&output)?;
            println!("{}x{}", image.width(), image.height());
        }
        "rd" => {
            let input = PathBuf::from(args.get(2).ok_or("input")?);
            let out_dir = PathBuf::from(args.get(3).unwrap_or(&"reports/frequency-rd".to_string()));
            std::fs::create_dir_all(&out_dir)?;
            let image = load(&input)?;
            let mut points = Vec::new();
            for step in [
                4.0f32, 6.0, 8.0, 10.0, 12.0, 16.0, 20.0, 24.0, 30.0, 40.0, 50.0, 60.0, 80.0, 100.0, 120.0,
                160.0, 200.0, 250.0, 300.0, 400.0, 500.0,
            ] {
                let bytes = encode(&image, step)?;
                let path = out_dir.join(format!("step_{step:.0}.rfi"));
                std::fs::write(&path, &bytes)?;
                let decoded = decode(&bytes)?;
                let value = psnr(&image, &decoded);
                points.push((step, bytes.len(), value));
                println!("step {step:.0}: {} bytes, {value:.2} dB", bytes.len());
            }
            for budget in [25_000usize, 50_000, 100_000, 200_000, 500_000, 1_000_000] {
                if let Some((step, size, value)) = points
                    .iter()
                    .filter(|(_, size, _)| *size <= budget)
                    .max_by_key(|(_, size, _)| *size)
                {
                    println!("BUDGET {budget}: step {step}, {size} bytes, {value:.2} dB");
                } else if let Some((step, size, value)) = points.iter().min_by_key(|(_, size, _)| *size) {
                    println!("BUDGET {budget}: min step {step}, {size} bytes, {value:.2} dB");
                }
            }
        }
        _ => eprintln!("unknown command"),
    }
    Ok(())
}
