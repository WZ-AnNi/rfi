use image::RgbaImage;
use std::path::PathBuf;

fn psnr(a: &RgbaImage, b: &RgbaImage) -> f64 {
    let mut sum = 0.0f64;
    for (left, right) in a.pixels().zip(b.pixels()) {
        for c in 0..4 {
            let d = f64::from(left.0[c]) - f64::from(right.0[c]);
            sum += d * d;
        }
    }
    let mse = sum / (a.width() as f64 * a.height() as f64 * 4.0);
    10.0 * (255.0f64 * 255.0 / mse).log10()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let input = PathBuf::from(args.get(1).ok_or("input image required")?);
    let out_dir = PathBuf::from(args.get(2).unwrap_or(&"reports/dct16-rd".to_string()));
    std::fs::create_dir_all(&out_dir)?;
    let image = image::open(input)?.to_rgba8();
    println!("| budget | qstep | bytes | RGB PSNR |");
    println!("|---:|---:|---:|---:|");
    for budget in [25_000usize, 50_000, 100_000, 200_000] {
        let (step, bytes) = rfi_frequency::encode16_to_budget(&image, budget)?;
        let decoded = rfi_frequency::decode16(&bytes)?;
        let value = psnr(&image, &decoded);
        std::fs::write(out_dir.join(format!("budget_{budget}.rfi")), &bytes)?;
        println!("| {budget} | {step:.2} | {} | {value:.2} |", bytes.len());
    }
    Ok(())
}
