use std::path::PathBuf;

use image::RgbaImage;
use rfi_frequency::analysis::analyze_bytes;
use rfi_frequency::metrics::{Quality, difference_map, quality};
use rfi_frequency::{decode, encode, encode_to_budget, inspect};

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

fn load(path: &PathBuf) -> Result<RgbaImage, Box<dyn std::error::Error>> {
    Ok(image::open(path)?.to_rgba8())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: rfi-cli encode|decode|inspect|rd ...");
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
            println!("{} bytes, qstep {step}", bytes.len());
        }
        "decode" => {
            let input = PathBuf::from(args.get(2).ok_or("input")?);
            let output = PathBuf::from(args.get(3).ok_or("output")?);
            let bytes = std::fs::read(&input)?;
            let image = decode(&bytes)?;
            image.save(&output)?;
            println!("{}x{}", image.width(), image.height());
        }
        "inspect" => {
            let input = PathBuf::from(args.get(2).ok_or("input")?);
            let bytes = std::fs::read(&input)?;
            let info = inspect(&bytes)?;
            println!(
                "RFI {}x{} tile={} basis={} qstep={} sections={} bytes={}",
                info.width,
                info.height,
                info.tile_size,
                info.basis,
                info.step,
                info.section_count,
                info.file_size
            );
        }

        "rans-rd" => {
            let input = PathBuf::from(args.get(2).ok_or("input")?);
            let out_dir = PathBuf::from(args.get(3).unwrap_or(&"reports/rans-rd".to_string()));
            std::fs::create_dir_all(&out_dir)?;
            let image = load(&input)?;
            println!("| budget | backend | basis | qstep | bytes | RGB | SSIM | MS-SSIM | blockiness |");
            println!("|---:|---|---|---:|---:|---:|---:|---:|---:|");
            for budget in [25_000usize, 50_000, 100_000, 200_000] {
                for (label, entropy) in [
                    ("zstd", rfi_frequency::Entropy::Zstd),
                    ("rANS", rfi_frequency::Entropy::Rans),
                ] {
                    let (step, bytes) = rfi_frequency::encode_to_budget_with(&image, budget, entropy)?;
                    let decoded = decode(&bytes)?;
                    let q = quality(&image, &decoded);
                    std::fs::write(out_dir.join(format!("{label}_{budget}.rfi")), &bytes)?;
                    println!(
                        "| {} | {} | DCT8 | {:.2} | {} | {:.2} | {:.4} | {:.4} | {:.3} |",
                        budget,
                        label,
                        step,
                        bytes.len(),
                        q.psnr_rgb,
                        q.ssim,
                        q.ms_ssim,
                        q.blockiness
                    );
                }
            }
            println!("wrote {}", out_dir.display());
        }
        "best-rd" => {
            let input = PathBuf::from(args.get(2).ok_or("input")?);
            let out_dir = PathBuf::from(args.get(3).unwrap_or(&"reports/best-rd".to_string()));
            std::fs::create_dir_all(&out_dir)?;
            let image = load(&input)?;
            println!("| budget | basis | qstep | bytes | RGB | SSIM | MS-SSIM | blockiness |");
            println!("|---:|---|---:|---:|---:|---:|---:|---:|");
            for budget in [25_000usize, 50_000, 100_000, 200_000] {
                // Evaluate every basis/backend pair at the same serialized budget.
                let mut candidates: Vec<(String, f32, Vec<u8>, image::RgbaImage, Quality)> = Vec::new();
                for (label, entropy) in [
                    ("DCT8+zstd", rfi_frequency::Entropy::Zstd),
                    ("DCT8+rANS", rfi_frequency::Entropy::Rans),
                    ("DCT8+auto", rfi_frequency::Entropy::Best),
                ] {
                    let (step, bytes) = rfi_frequency::encode_to_budget_with(&image, budget, entropy)?;
                    let decoded = decode(&bytes)?;
                    let q = quality(&image, &decoded);
                    candidates.push((label.to_string(), step, bytes, decoded, q));
                }
                for (label, entropy) in [
                    ("DCT16+zstd", rfi_frequency::Entropy::Zstd),
                    ("DCT16+rANS", rfi_frequency::Entropy::Rans),
                    ("DCT16+auto", rfi_frequency::Entropy::Best),
                ] {
                    let (step, bytes) = rfi_frequency::encode16_to_budget_with(&image, budget, entropy)?;
                    let decoded = decode(&bytes)?;
                    let q = quality(&image, &decoded);
                    candidates.push((label.to_string(), step, bytes, decoded, q));
                }
                let best_index = candidates
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.4.psnr_rgb.total_cmp(&b.1.4.psnr_rgb))
                    .map(|(index, _)| index)
                    .expect("candidates");
                let (label, step, bytes, decoded, q) = candidates.swap_remove(best_index);
                std::fs::write(out_dir.join(format!("budget_{budget}.rfi")), &bytes)?;
                difference_map(&image, &decoded).save(out_dir.join(format!("budget_{budget}_diff.png")))?;
                println!(
                    "| {} | {} | {:.2} | {} | {:.2} | {:.4} | {:.4} | {:.3} |",
                    budget,
                    label,
                    step,
                    bytes.len(),
                    q.psnr_rgb,
                    q.ssim,
                    q.ms_ssim,
                    q.blockiness
                );
            }
            println!("wrote {}", out_dir.display());
        }
        "dct16-rd" => {
            let input = PathBuf::from(args.get(2).ok_or("input")?);
            let out_dir = PathBuf::from(args.get(3).unwrap_or(&"reports/dct16-rd".to_string()));
            std::fs::create_dir_all(&out_dir)?;
            let image = load(&input)?;
            println!("| budget | qstep | bytes | RGB | Y | Cb | Cr | SSIM | MS-SSIM | blockiness |");
            println!("|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
            for budget in [25_000usize, 50_000, 100_000, 200_000] {
                let (step, bytes) = rfi_frequency::encode16_to_budget(&image, budget)?;
                let decoded = decode(&bytes)?;
                let q = quality(&image, &decoded);
                std::fs::write(out_dir.join(format!("budget_{budget}.rfi")), &bytes)?;
                difference_map(&image, &decoded).save(out_dir.join(format!("budget_{budget}_diff.png")))?;
                println!(
                    "| {} | {:.2} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.4} | {:.4} | {:.3} |",
                    budget,
                    step,
                    bytes.len(),
                    q.psnr_rgb,
                    q.psnr_y,
                    q.psnr_cb,
                    q.psnr_cr,
                    q.ssim,
                    q.ms_ssim,
                    q.blockiness
                );
            }
            println!("wrote {}", out_dir.display());
        }
        "analyze" => {
            let input = PathBuf::from(args.get(2).ok_or("input")?);
            let out_dir = PathBuf::from(args.get(3).unwrap_or(&"reports/p0".to_string()));
            std::fs::create_dir_all(&out_dir)?;
            let image = load(&input)?;
            let budgets = [25_000usize, 50_000, 100_000, 200_000];
            let mut summary = String::new();
            summary.push_str("# RFI P0 coefficient analysis\n\n");
            summary.push_str("True rate control: qstep is searched by final serialized bytes.\n\n");
            summary
                .push_str("| budget | qstep | bytes | RGB | Y | Cb | Cr | SSIM | MS-SSIM | blockiness |\n");
            summary.push_str("|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
            let mut channel_csv = String::from(
                "budget,qstep,channel,blocks,raw_bytes,zstd_bytes,dc_zstd,ac_zstd,dc_bytes,ac_bytes,nonzero_ratio,avg_zero_run,dc_abs,ac_abs,pos,neg\n",
            );
            let mut tile_csv = String::from(
                "budget,tile_x,tile_y,y_energy,cb_energy,cr_energy,nonzero_ratio,avg_zero_run,raw_bytes,stored_estimate,mse\n",
            );
            let mut section_md = String::from(
                "\n## Section bytes\n\n| budget | header+table | basis | tile stored/raw | coeff stored/raw |\n|---:|---:|---:|---:|---:|\n",
            );
            let mut channel_md = String::from(
                "\n## Channel zstd bytes\n\n| budget | channel | raw | zstd | dc zstd | ac zstd | nonzero | avg zero run |\n|---:|---|---:|---:|---:|---:|---:|---:|\n",
            );
            let mut freq_csv = String::from("budget,channel,freq,nonzero\n");
            for budget in budgets {
                let (step, bytes) = encode_to_budget(&image, budget)?;
                let analysis = analyze_bytes(&image, step, &bytes)?;
                let decoded = decode(&bytes)?;
                let quality = quality(&image, &decoded);
                let name = format!("budget_{budget}");
                std::fs::write(out_dir.join(format!("{name}.rfi")), &bytes)?;
                difference_map(&image, &decoded).save(out_dir.join(format!("{name}_diff.png")))?;
                println!(
                    "budget {budget}: qstep {step:.2}, {} bytes, RGB {:.2} dB, SSIM {:.4}, MS-SSIM {:.4}, blockiness {:.3}",
                    bytes.len(),
                    quality.psnr_rgb,
                    quality.ssim,
                    quality.ms_ssim,
                    quality.blockiness
                );
                summary.push_str(&format!(
                    "| {} | {:.2} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.4} | {:.4} | {:.3} |\n",
                    budget,
                    step,
                    bytes.len(),
                    quality.psnr_rgb,
                    quality.psnr_y,
                    quality.psnr_cb,
                    quality.psnr_cr,
                    quality.ssim,
                    quality.ms_ssim,
                    quality.blockiness
                ));
                for (channel, stats) in analysis.channels.iter().enumerate() {
                    let name = match channel {
                        0 => "Y",
                        1 => "Cb",
                        _ => "Cr",
                    };
                    channel_csv.push_str(&format!(
                        "{budget},{step:.2},{name},{},{},{},{},{},{},{},{:.5},{:.2},{:.1},{:.1},{},{}\n",
                        stats.blocks,
                        stats.raw_bytes,
                        stats.zstd_bytes,
                        stats.dc_zstd_bytes,
                        stats.ac_zstd_bytes,
                        stats.dc_bytes,
                        stats.ac_bytes,
                        stats.nonzero_ratio(),
                        stats.average_zero_run(),
                        stats.dc_abs_sum,
                        stats.ac_abs_sum,
                        stats.positive,
                        stats.negative
                    ));
                }
                section_md.push_str(&format!(
                    "| {} | {} | {} | {} / {} | {} / {} |\n",
                    budget,
                    analysis.sections.header_bytes + analysis.sections.section_table_bytes,
                    analysis.sections.basis_stored,
                    analysis.sections.tile_directory_stored,
                    analysis.sections.tile_directory_raw,
                    analysis.sections.coefficients_stored,
                    analysis.sections.coefficients_raw
                ));
                for (channel, stats) in analysis.channels.iter().enumerate() {
                    let name = match channel {
                        0 => "Y",
                        1 => "Cb",
                        _ => "Cr",
                    };
                    channel_md.push_str(&format!(
                        "| {} | {} | {} | {} | {} | {} | {:.5} | {:.2} |\n",
                        budget,
                        name,
                        stats.raw_bytes,
                        stats.zstd_bytes,
                        stats.dc_zstd_bytes,
                        stats.ac_zstd_bytes,
                        stats.nonzero_ratio(),
                        stats.average_zero_run()
                    ));
                    for (freq, nonzero) in stats.freq_nonzero.iter().enumerate() {
                        freq_csv.push_str(&format!("{budget},{name},{freq},{nonzero}\n"));
                    }
                }
                for tile in &analysis.tiles {
                    tile_csv.push_str(&format!(
                        "{budget},{},{},{:.1},{:.1},{:.1},{:.5},{:.2},{},{},{:.3}\n",
                        tile.tile_x,
                        tile.tile_y,
                        tile.y_ac_energy,
                        tile.cb_ac_energy,
                        tile.cr_ac_energy,
                        tile.nonzero_ratio,
                        tile.average_zero_run,
                        tile.raw_bytes,
                        tile.stored_bytes_estimate,
                        tile.mse
                    ));
                }
            }
            summary.push_str(&section_md);
            summary.push_str(&channel_md);
            std::fs::write(out_dir.join("p0.md"), &summary)?;
            std::fs::write(out_dir.join("channels.csv"), channel_csv)?;
            std::fs::write(out_dir.join("freq.csv"), freq_csv)?;
            std::fs::write(out_dir.join("tiles.csv"), tile_csv)?;
            println!("wrote {}", out_dir.display());
        }
        "rd" => {
            let input = PathBuf::from(args.get(2).ok_or("input")?);
            let out_dir = PathBuf::from(args.get(3).unwrap_or(&"reports/rfi-rd".to_string()));
            std::fs::create_dir_all(&out_dir)?;
            let image = load(&input)?;
            let mut points = Vec::new();
            for step in [
                4.0f32, 6.0, 8.0, 10.0, 12.0, 16.0, 20.0, 24.0, 30.0, 40.0, 50.0, 60.0, 80.0, 100.0, 120.0,
                160.0, 200.0, 250.0, 300.0, 400.0, 500.0,
            ] {
                let bytes = encode(&image, step)?;
                std::fs::write(out_dir.join(format!("step_{step:.0}.rfi")), &bytes)?;
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
                }
            }
        }
        other => eprintln!("unknown command {other}"),
    }
    Ok(())
}
