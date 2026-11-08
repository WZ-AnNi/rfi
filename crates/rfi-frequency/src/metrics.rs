//! P0 quality metrics: PSNR by channel, SSIM, MS-SSIM, blockiness.

use image::{Rgba, RgbaImage};

#[derive(Debug, Clone, Copy, Default)]
pub struct Quality {
    pub psnr_rgb: f64,
    pub psnr_y: f64,
    pub psnr_cb: f64,
    pub psnr_cr: f64,
    pub ssim: f64,
    pub ms_ssim: f64,
    pub blockiness: f64,
}

fn to_ycbcr(image: &RgbaImage) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let n = (image.width() * image.height()) as usize;
    let mut y = Vec::with_capacity(n);
    let mut cb = Vec::with_capacity(n);
    let mut cr = Vec::with_capacity(n);
    for p in image.pixels() {
        let r = f32::from(p.0[0]);
        let g = f32::from(p.0[1]);
        let b = f32::from(p.0[2]);
        y.push(0.299 * r + 0.587 * g + 0.114 * b);
        cb.push(-0.168736 * r - 0.331264 * g + 0.5 * b + 128.0);
        cr.push(0.5 * r - 0.418688 * g - 0.081312 * b + 128.0);
    }
    (y, cb, cr)
}

fn psnr_plane(a: &[f32], b: &[f32]) -> f64 {
    if a.is_empty() {
        return f64::INFINITY;
    }
    let mut sum = 0.0f64;
    for (x, y) in a.iter().zip(b) {
        let diff = f64::from(*x) - f64::from(*y);
        sum += diff * diff;
    }
    let mse = sum / a.len() as f64;
    if mse <= 0.0 {
        f64::INFINITY
    } else {
        10.0 * (255.0f64 * 255.0 / mse).log10()
    }
}

fn psnr_rgba(a: &RgbaImage, b: &RgbaImage) -> f64 {
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

fn ssim_plane(a: &[f32], b: &[f32], width: usize, height: usize) -> f64 {
    if width < 8 || height < 8 {
        return 1.0;
    }
    let c1 = 6.5025f64;
    let c2 = 58.5225f64;
    let mut total = 0.0f64;
    let mut count = 0u64;
    let mut y = 0usize;
    while y + 8 <= height {
        let mut x = 0usize;
        while x + 8 <= width {
            let mut sum_a = 0.0f64;
            let mut sum_b = 0.0f64;
            let mut sum_aa = 0.0f64;
            let mut sum_bb = 0.0f64;
            let mut sum_ab = 0.0f64;
            for yy in 0..8 {
                for xx in 0..8 {
                    let index = (y + yy) * width + x + xx;
                    let va = f64::from(a[index]);
                    let vb = f64::from(b[index]);
                    sum_a += va;
                    sum_b += vb;
                    sum_aa += va * va;
                    sum_bb += vb * vb;
                    sum_ab += va * vb;
                }
            }
            let n = 64.0;
            let mean_a = sum_a / n;
            let mean_b = sum_b / n;
            let var_a = sum_aa / n - mean_a * mean_a;
            let var_b = sum_bb / n - mean_b * mean_b;
            let cov = sum_ab / n - mean_a * mean_b;
            let numerator = (2.0 * mean_a * mean_b + c1) * (2.0 * cov + c2);
            let denominator = (mean_a * mean_a + mean_b * mean_b + c1) * (var_a + var_b + c2);
            total += numerator / denominator;
            count += 1;
            x += 8;
        }
        y += 8;
    }
    if count == 0 { 1.0 } else { total / count as f64 }
}

fn downsample(values: &[f32], width: usize, height: usize) -> (Vec<f32>, usize, usize) {
    let out_width = width / 2;
    let out_height = height / 2;
    let mut out = vec![0.0f32; out_width * out_height];
    for y in 0..out_height {
        for x in 0..out_width {
            let index = (y * 2) * width + x * 2;
            out[y * out_width + x] =
                (values[index] + values[index + 1] + values[index + width] + values[index + width + 1]) / 4.0;
        }
    }
    (out, out_width, out_height)
}

fn ms_ssim_plane(a: &[f32], b: &[f32], width: usize, height: usize) -> f64 {
    let weights = [0.0448f64, 0.2856, 0.3001, 0.2363, 0.1333];
    let mut a = a.to_vec();
    let mut b = b.to_vec();
    let mut w = width;
    let mut h = height;
    let mut product = 1.0f64;
    let mut used = 0.0f64;
    for weight in weights {
        if w < 8 || h < 8 {
            break;
        }
        product *= ssim_plane(&a, &b, w, h).max(1e-9).powf(weight);
        used += weight;
        if w < 16 || h < 16 {
            break;
        }
        let (na, nw, nh) = downsample(&a, w, h);
        let (nb, _, _) = downsample(&b, w, h);
        a = na;
        b = nb;
        w = nw;
        h = nh;
    }
    if used <= 0.0 {
        1.0
    } else {
        product.powf(1.0 / used)
    }
}

fn blockiness(y: &[f32], width: usize, height: usize) -> f64 {
    let mut boundary = 0.0f64;
    let mut boundary_count = 0u64;
    let mut interior = 0.0f64;
    let mut interior_count = 0u64;
    for yy in 0..height {
        for xx in 1..width {
            let diff = (f64::from(y[yy * width + xx]) - f64::from(y[yy * width + xx - 1])).abs();
            if xx % 8 == 0 {
                boundary += diff;
                boundary_count += 1;
            } else {
                interior += diff;
                interior_count += 1;
            }
        }
    }
    for xx in 0..width {
        for yy in 1..height {
            let diff = (f64::from(y[yy * width + xx]) - f64::from(y[(yy - 1) * width + xx])).abs();
            if yy % 8 == 0 {
                boundary += diff;
                boundary_count += 1;
            } else {
                interior += diff;
                interior_count += 1;
            }
        }
    }
    let b = boundary / boundary_count.max(1) as f64;
    let i = interior / interior_count.max(1) as f64;
    if i <= 1e-9 { 1.0 } else { b / i }
}

pub fn quality(source: &RgbaImage, decoded: &RgbaImage) -> Quality {
    let (y_a, cb_a, cr_a) = to_ycbcr(source);
    let (y_b, cb_b, cr_b) = to_ycbcr(decoded);
    let width = source.width() as usize;
    let height = source.height() as usize;
    Quality {
        psnr_rgb: psnr_rgba(source, decoded),
        psnr_y: psnr_plane(&y_a, &y_b),
        psnr_cb: psnr_plane(&cb_a, &cb_b),
        psnr_cr: psnr_plane(&cr_a, &cr_b),
        ssim: ssim_plane(&y_a, &y_b, width, height),
        ms_ssim: ms_ssim_plane(&y_a, &y_b, width, height),
        blockiness: blockiness(&y_b, width, height),
    }
}

pub fn difference_map(source: &RgbaImage, decoded: &RgbaImage) -> RgbaImage {
    let mut out = RgbaImage::new(source.width(), source.height());
    for (x, y, pixel) in out.enumerate_pixels_mut() {
        let a = source.get_pixel(x, y).0;
        let b = decoded.get_pixel(x, y).0;
        let diff = (0..3)
            .map(|c| (i32::from(a[c]) - i32::from(b[c])).unsigned_abs() as u8)
            .max()
            .unwrap_or(0);
        let value = diff.saturating_mul(4);
        *pixel = Rgba([value, value, value, 255]);
    }
    out
}
