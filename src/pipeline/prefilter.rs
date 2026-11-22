use image::{Rgba, RgbaImage};

use crate::state::SourceMode;

pub fn apply_prefilter(
    src: &RgbaImage,
    source_mode: SourceMode,
    brightness: f32,
    contrast: f32,
    saturation: f32,
    alpha_threshold: bool,
) -> RgbaImage {
    let w = src.width();
    let h = src.height();
    let has_color_adj =
        brightness.abs() > 1e-4 || contrast.abs() > 1e-4 || saturation.abs() > 1e-4;

    let mut out = if !has_color_adj && !alpha_threshold {
        src.clone()
    } else {
        let mut buf = RgbaImage::new(w, h);
        let contrast_factor = if contrast >= 0.0 {
            1.0 + contrast * 2.0
        } else {
            (1.0 + contrast).max(0.05)
        };
        let sat_factor = (1.0 + saturation).max(0.0);

        for (dst_p, src_p) in buf.pixels_mut().zip(src.pixels()) {
            let mut a = src_p[3];
            if alpha_threshold {
                if a < 128 {
                    *dst_p = Rgba([0, 0, 0, 0]);
                    continue;
                }
                a = 255;
            } else if a == 0 {
                *dst_p = Rgba([0, 0, 0, 0]);
                continue;
            }

            let mut r = src_p[0] as f32 / 255.0;
            let mut g = src_p[1] as f32 / 255.0;
            let mut b = src_p[2] as f32 / 255.0;

            if has_color_adj {
                r += brightness * 0.5;
                g += brightness * 0.5;
                b += brightness * 0.5;

                r = (r - 0.5) * contrast_factor + 0.5;
                g = (g - 0.5) * contrast_factor + 0.5;
                b = (b - 0.5) * contrast_factor + 0.5;

                let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                r = luma + (r - luma) * sat_factor;
                g = luma + (g - luma) * sat_factor;
                b = luma + (b - luma) * sat_factor;
            }

            *dst_p = Rgba([
                (r.clamp(0.0, 1.0) * 255.0).round() as u8,
                (g.clamp(0.0, 1.0) * 255.0).round() as u8,
                (b.clamp(0.0, 1.0) * 255.0).round() as u8,
                a,
            ]);
        }
        buf
    };

    if source_mode == SourceMode::Photo {
        out = bilateral_smooth(&out);
    }

    out
}

/// Light 5x5 edge-preserving bilateral filter for photo inputs.
fn bilateral_smooth(src: &RgbaImage) -> RgbaImage {
    let w = src.width() as i32;
    let h = src.height() as i32;
    let mut dst = src.clone();

    let s_raw = src.as_raw();
    let d_raw = dst.as_mut();

    let spatial_denom = 2.0 * 1.5 * 1.5;
    let range_denom = 2.0 * 0.12 * 0.12;

    let mut spatial_weights = [[0.0f32; 5]; 5];
    for dy in -2..=2 {
        for dx in -2..=2 {
            let d2 = (dx * dx + dy * dy) as f32;
            spatial_weights[(dy + 2) as usize][(dx + 2) as usize] = (-d2 / spatial_denom).exp();
        }
    }

    for y in 0..h {
        for x in 0..w {
            let c_idx = ((y * w + x) * 4) as usize;
            if s_raw[c_idx + 3] < 128 {
                continue;
            }

            let cr = s_raw[c_idx] as f32 / 255.0;
            let cg = s_raw[c_idx + 1] as f32 / 255.0;
            let cb = s_raw[c_idx + 2] as f32 / 255.0;

            let mut sum_w = 0.0f32;
            let mut sum_r = 0.0f32;
            let mut sum_g = 0.0f32;
            let mut sum_b = 0.0f32;

            for dy in -2..=2 {
                let ny = y + dy;
                if ny < 0 || ny >= h {
                    continue;
                }
                for dx in -2..=2 {
                    let nx = x + dx;
                    if nx < 0 || nx >= w {
                        continue;
                    }
                    let n_idx = ((ny * w + nx) * 4) as usize;
                    if s_raw[n_idx + 3] < 128 {
                        continue;
                    }
                    let nr = s_raw[n_idx] as f32 / 255.0;
                    let ng = s_raw[n_idx + 1] as f32 / 255.0;
                    let nb = s_raw[n_idx + 2] as f32 / 255.0;

                    let dr = nr - cr;
                    let dg = ng - cg;
                    let db = nb - cb;
                    let range_w = (-(dr * dr + dg * dg + db * db) / range_denom).exp();
                    let w_val = spatial_weights[(dy + 2) as usize][(dx + 2) as usize] * range_w;

                    sum_w += w_val;
                    sum_r += nr * w_val;
                    sum_g += ng * w_val;
                    sum_b += nb * w_val;
                }
            }

            if sum_w > 0.0 {
                d_raw[c_idx] = ((sum_r / sum_w).clamp(0.0, 1.0) * 255.0).round() as u8;
                d_raw[c_idx + 1] = ((sum_g / sum_w).clamp(0.0, 1.0) * 255.0).round() as u8;
                d_raw[c_idx + 2] = ((sum_b / sum_w).clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
    }

    dst
}
