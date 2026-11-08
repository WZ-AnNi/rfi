//! Measures the theoretical ceiling of alternative 8x8 transform bases.
//!
//! Trains a KLT on the actual image blocks, then compares DCT8 / KLT8 / DST8
//! at matched quantization steps. Basis transmission cost is NOT included here:
//! this answers only whether a stronger basis is worth implementing.
//!
//! WHY: this is a numeric measurement harness; index-based matrix loops are the
//! clearest form here, so the usual iterator-style lints are allowed locally.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]
use image::RgbaImage;

const N: usize = 8;
const NN: usize = N * N;

type Mat = [[f32; NN]; NN];

fn dct_basis() -> Mat {
    let mut one = [[0.0f32; N]; N];
    for (u, row) in one.iter_mut().enumerate() {
        let cu = if u == 0 {
            (1.0 / N as f32).sqrt()
        } else {
            (2.0 / N as f32).sqrt()
        };
        for (x, value) in row.iter_mut().enumerate() {
            *value = cu * (((2 * x + 1) as f32) * u as f32 * std::f32::consts::PI / (2.0 * N as f32)).cos();
        }
    }
    let mut basis = [[0.0f32; NN]; NN];
    for v in 0..N {
        for u in 0..N {
            for y in 0..N {
                for x in 0..N {
                    basis[v * N + u][y * N + x] = one[v][y] * one[u][x];
                }
            }
        }
    }
    basis
}
fn dst_basis() -> Mat {
    let mut one = [[0.0f32; N]; N];
    for u in 0..N {
        let cu = (2.0 / (N as f32 + 1.0)).sqrt();
        for x in 0..N {
            one[u][x] =
                cu * ((((x + 1) as f32) * ((u + 1) as f32) * std::f32::consts::PI) / (N as f32 + 1.0)).sin();
        }
    }
    let mut basis = [[0.0f32; NN]; NN];
    for v in 0..N {
        for u in 0..N {
            for y in 0..N {
                for x in 0..N {
                    basis[v * N + u][y * N + x] = one[v][y] * one[u][x];
                }
            }
        }
    }
    basis
}

/// Jacobi eigenvalue iteration for a symmetric matrix.
fn jacobi(mut a: Mat) -> (Vec<f32>, Mat) {
    let mut v = [[0.0f32; NN]; NN];
    for i in 0..NN {
        v[i][i] = 1.0;
    }
    for _sweep in 0..32 {
        let mut off = 0.0f32;
        for p in 0..NN {
            for q in (p + 1)..NN {
                off += a[p][q] * a[p][q];
            }
        }
        if off < 1e-12 {
            break;
        }
        for p in 0..NN {
            for q in (p + 1)..NN {
                if a[p][q].abs() < 1e-14 {
                    continue;
                }
                let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..NN {
                    let akp = a[k][p];
                    let akq = a[k][q];
                    a[k][p] = c * akp - s * akq;
                    a[k][q] = s * akp + c * akq;
                }
                for k in 0..NN {
                    let apk = a[p][k];
                    let aqk = a[q][k];
                    a[p][k] = c * apk - s * aqk;
                    a[q][k] = s * apk + c * aqk;
                }
                for k in 0..NN {
                    let vkp = v[k][p];
                    let vkq = v[k][q];
                    v[k][p] = c * vkp - s * vkq;
                    v[k][q] = s * vkp + c * vkq;
                }
            }
        }
    }
    let mut eigenvalues = vec![0.0f32; NN];
    for i in 0..NN {
        eigenvalues[i] = a[i][i];
    }
    (eigenvalues, v)
}

fn klt_basis(samples: &[[f32; NN]]) -> Mat {
    let mut cov = [[0.0f64; NN]; NN];
    for sample in samples {
        for i in 0..NN {
            for j in 0..NN {
                cov[i][j] += f64::from(sample[i]) * f64::from(sample[j]);
            }
        }
    }
    let n = samples.len().max(1) as f64;
    let mut asym = [[0.0f32; NN]; NN];
    for i in 0..NN {
        for j in 0..NN {
            asym[i][j] = (cov[i][j] / n) as f32;
        }
    }
    let (eigenvalues, mut vectors) = jacobi(asym);
    let mut order: Vec<usize> = (0..NN).collect();
    order.sort_by(|a, b| eigenvalues[*b].total_cmp(&eigenvalues[*a]));
    let mut basis = [[0.0f32; NN]; NN];
    for (rank, index) in order.iter().enumerate() {
        let mut norm = 0.0f32;
        for i in 0..NN {
            norm += vectors[i][*index] * vectors[i][*index];
        }
        let norm = norm.sqrt().max(1e-12);
        for i in 0..NN {
            basis[rank][i] = vectors[i][*index] / norm;
        }
    }
    vectors = basis;
    vectors
}

fn forward(basis: &Mat, block: &[f32; NN]) -> [f32; NN] {
    let mut out = [0.0f32; NN];
    for k in 0..NN {
        let mut sum = 0.0f32;
        for i in 0..NN {
            sum += basis[k][i] * block[i];
        }
        out[k] = sum;
    }
    out
}

fn inverse(basis: &Mat, coeff: &[f32; NN]) -> [f32; NN] {
    let mut out = [0.0f32; NN];
    for i in 0..NN {
        let mut sum = 0.0f32;
        for k in 0..NN {
            sum += basis[k][i] * coeff[k];
        }
        out[i] = sum;
    }
    out
}

fn psnr_from_mse(mse: f64) -> f64 {
    if mse <= 0.0 {
        f64::INFINITY
    } else {
        10.0 * (255.0f64 * 255.0 / mse).log10()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let input = args.get(1).ok_or("input image required")?;
    let image: RgbaImage = image::open(input)?.to_rgba8();
    let width = image.width();
    let height = image.height();

    // Gather mean-removed 8x8 luma blocks.
    let mut samples: Vec<[f32; NN]> = Vec::new();
    let mut positions: Vec<(u32, u32)> = Vec::new();
    let mut y = 0u32;
    while y + N as u32 <= height {
        let mut x = 0u32;
        while x + N as u32 <= width {
            let mut block = [0.0f32; NN];
            let mut mean = 0.0f32;
            for by in 0..N {
                for bx in 0..N {
                    let p = image.get_pixel(x + bx as u32, y + by as u32).0;
                    let luma = 0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2]);
                    block[by * N + bx] = luma - 128.0;
                    mean += luma - 128.0;
                }
            }
            mean /= NN as f32;
            for value in block.iter_mut() {
                *value -= mean;
            }
            samples.push(block);
            positions.push((x, y));
            x += N as u32;
        }
        y += N as u32;
    }
    let mean_removed: Vec<f32> = positions
        .iter()
        .map(|(x, y)| {
            let mut sum = 0.0f32;
            for by in 0..N {
                for bx in 0..N {
                    let p = image.get_pixel(x + bx as u32, y + by as u32).0;
                    sum += 0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2]);
                }
            }
            sum / NN as f32
        })
        .collect();

    let dct = dct_basis();
    let dst = dst_basis();
    let klt = klt_basis(&samples);

    // Energy compaction: fraction of AC energy in the first k coefficients.
    let compaction = |basis: &Mat, label: &str| {
        let mut total = 0.0f64;
        let mut kept = [0.0f64; NN];
        for sample in &samples {
            let coeff = forward(basis, sample);
            for (k, value) in coeff.iter().enumerate() {
                let energy = f64::from(*value) * f64::from(*value);
                total += energy;
                kept[k] += energy;
            }
        }
        let mut running = 0.0f64;
        print!("{label:>6} energy share by rank:");
        for (k, value) in kept.iter().enumerate() {
            running += value;
            if matches!(k, 0 | 3 | 7 | 15 | 31 | 63) {
                print!("  top{:<2} {:>5.1}%", k + 1, 100.0 * running / total);
            }
        }
        println!();
    };
    compaction(&dct, "DCT8");
    compaction(&dst, "DST8");
    compaction(&klt, "KLT8");

    println!();
    println!("| step | DCT8 | DST8 | KLT8 |");
    println!("|---:|---:|---:|---:|");
    for step in [4.0f32, 8.0, 12.0, 20.0, 32.0, 48.0] {
        let mut scores = Vec::new();
        for basis in [&dct, &dst, &klt] {
            let mut mse = 0.0f64;
            for (index, sample) in samples.iter().enumerate() {
                let coeff = forward(basis, sample);
                let mut quantized = [0.0f32; NN];
                for k in 0..NN {
                    quantized[k] = (coeff[k] / step).round() * step;
                }
                let restored = inverse(basis, &quantized);
                let mean = mean_removed[index];
                for k in 0..NN {
                    let original = sample[k] + mean;
                    let decoded = restored[k] + mean;
                    let diff = f64::from(original) - f64::from(decoded);
                    mse += diff * diff;
                }
            }
            mse /= (samples.len() * NN) as f64;
            scores.push(psnr_from_mse(mse));
        }
        println!(
            "| {step:.0} | {:.2} | {:.2} | {:.2} |",
            scores[0], scores[1], scores[2]
        );
    }

    // Transform coding gain: the standard metric for basis strength.
    // G = 10*log10( arithmetic_mean(var_k) / geometric_mean(var_k) ).
    // A higher gain means the same distortion is reachable at a lower rate when
    // each coefficient gets a quantization step matched to its own variance.
    println!();
    println!("| basis | transform coding gain |");
    println!("|---|---:|");
    let mut gains = Vec::new();
    for (label, basis) in [("DCT8", &dct), ("DST8", &dst), ("KLT8", &klt)] {
        let mut variance = [0.0f64; NN];
        for sample in &samples {
            let coeff = forward(basis, sample);
            for k in 0..NN {
                variance[k] += f64::from(coeff[k]) * f64::from(coeff[k]);
            }
        }
        let n = samples.len().max(1) as f64;
        for value in variance.iter_mut() {
            *value = (*value / n).max(1e-12);
        }
        let arithmetic = variance.iter().sum::<f64>() / NN as f64;
        let geometric = (variance.iter().map(|v| v.ln()).sum::<f64>() / NN as f64).exp();
        let gain = 10.0 * (arithmetic / geometric).log10();
        gains.push(gain);
        println!("| {label} | {gain:.2} dB |");
    }

    // Matched-rate test: per-coefficient steps proportional to sqrt(variance).
    println!();
    println!("| step | DCT8 | DST8 | KLT8 |");
    println!("|---:|---:|---:|---:|");
    let mut steps = Vec::new();
    for basis in [&dct, &dst, &klt] {
        let mut variance = [0.0f64; NN];
        for sample in &samples {
            let coeff = forward(basis, sample);
            for k in 0..NN {
                variance[k] += f64::from(coeff[k]) * f64::from(coeff[k]);
            }
        }
        let n = samples.len().max(1) as f64;
        let mut sigma = [0.0f32; NN];
        for k in 0..NN {
            sigma[k] = ((variance[k] / n).max(1e-12)).sqrt() as f32;
        }
        let geo = (sigma.iter().map(|v| (*v as f64).ln()).sum::<f64>() / NN as f64).exp() as f32;
        let mut scaled = [1.0f32; NN];
        for k in 0..NN {
            scaled[k] = sigma[k] / geo;
        }
        steps.push(scaled);
    }
    for step in [1.0f32, 2.0, 4.0, 8.0, 16.0] {
        let mut scores = Vec::new();
        for (index, basis) in [&dct, &dst, &klt].iter().enumerate() {
            let scale = &steps[index];
            let mut mse = 0.0f64;
            for (block_index, sample) in samples.iter().enumerate() {
                let coeff = forward(basis, sample);
                let mut quantized = [0.0f32; NN];
                for k in 0..NN {
                    let local = step * scale[k];
                    quantized[k] = (coeff[k] / local).round() * local;
                }
                let restored = inverse(basis, &quantized);
                let mean = mean_removed[block_index];
                for k in 0..NN {
                    let diff = f64::from(sample[k] + mean) - f64::from(restored[k] + mean);
                    mse += diff * diff;
                }
            }
            mse /= (samples.len() * NN) as f64;
            scores.push(psnr_from_mse(mse));
        }
        println!(
            "| {step:.0} | {:.2} | {:.2} | {:.2} |",
            scores[0], scores[1], scores[2]
        );
    }

    // Honest check: train the KLT on one half of the blocks, score on the other.
    println!();
    println!("held-out KLT (trained on even blocks, scored on odd blocks):");
    let train: Vec<[f32; NN]> = samples.iter().step_by(2).copied().collect();
    if train.len() > 64 {
        let held_klt = klt_basis(&train);
        let mut variance = [0.0f64; NN];
        for sample in &train {
            let coeff = forward(&held_klt, sample);
            for k in 0..NN {
                variance[k] += f64::from(coeff[k]) * f64::from(coeff[k]);
            }
        }
        let tn = train.len() as f64;
        let mut sigma = [0.0f32; NN];
        for k in 0..NN {
            sigma[k] = ((variance[k] / tn).max(1e-12)).sqrt() as f32;
        }
        let geo = (sigma.iter().map(|v| (*v as f64).ln()).sum::<f64>() / NN as f64).exp() as f32;
        let mut scale = [1.0f32; NN];
        for k in 0..NN {
            scale[k] = sigma[k] / geo;
        }
        for (label, basis) in [("DCT8", &dct), ("KLT8-held", &held_klt)] {
            let mut best = 0.0f64;
            for step in [1.0f32, 2.0, 4.0, 8.0, 16.0] {
                let mut mse = 0.0f64;
                let mut count = 0usize;
                for (block_index, sample) in samples.iter().enumerate() {
                    if block_index % 2 == 0 {
                        continue;
                    }
                    let coeff = forward(basis, sample);
                    let mut quantized = [0.0f32; NN];
                    for k in 0..NN {
                        let local = step * scale[k];
                        quantized[k] = (coeff[k] / local).round() * local;
                    }
                    let restored = inverse(basis, &quantized);
                    let mean = mean_removed[block_index];
                    for k in 0..NN {
                        let diff = f64::from(sample[k] + mean) - f64::from(restored[k] + mean);
                        mse += diff * diff;
                    }
                    count += NN;
                }
                mse /= count.max(1) as f64;
                let value = psnr_from_mse(mse);
                if value > best {
                    best = value;
                }
            }
            println!("  {label}: best held-out PSNR {best:.2} dB");
        }
    }

    Ok(())
}
