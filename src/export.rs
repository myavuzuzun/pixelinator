use std::path::Path;

use image::{ImageFormat, RgbaImage};

/// Nearest-neighbor integer upscaling so pixel art stays razor sharp.
pub fn upscale_nearest(src: &RgbaImage, scale: u32) -> RgbaImage {
    let factor = scale.max(1);
    if factor == 1 {
        return src.clone();
    }

    let sw = src.width();
    let sh = src.height();
    let dw = sw.saturating_mul(factor);
    let dh = sh.saturating_mul(factor);

    let mut dst = RgbaImage::new(dw, dh);
    let s_raw = src.as_raw();
    let d_raw = dst.as_mut();
    let f = factor as usize;
    let sw_usize = sw as usize;
    let dw_usize = dw as usize;

    for sy in 0..sh as usize {
        let s_row_start = sy * sw_usize * 4;
        for sx in 0..sw_usize {
            let sp = &s_raw[s_row_start + sx * 4..s_row_start + sx * 4 + 4];
            let dx_base = sx * f;
            for oy in 0..f {
                let d_row_start = (sy * f + oy) * dw_usize * 4;
                for ox in 0..f {
                    let dp = d_row_start + (dx_base + ox) * 4;
                    d_raw[dp..dp + 4].copy_from_slice(sp);
                }
            }
        }
    }

    dst
}

pub fn save_png(img: &RgbaImage, scale: u32, path: &Path) -> Result<(), String> {
    let upscaled = upscale_nearest(img, scale);
    upscaled
        .save_with_format(path, ImageFormat::Png)
        .map_err(|e| format!("failed to save PNG: {e}"))
}
