use image::{Rgba, RgbaImage};

use crate::pipeline::color::{srgb_u8_to_oklab, Oklab};
use crate::state::DitherMode;

const BAYER_2X2: [[f32; 2]; 2] = [[0.0, 2.0], [3.0, 1.0]];

const BAYER_4X4: [[f32; 4]; 4] = [
    [0.0, 8.0, 2.0, 10.0],
    [12.0, 4.0, 14.0, 6.0],
    [3.0, 11.0, 1.0, 9.0],
    [15.0, 7.0, 13.0, 5.0],
];

#[inline]
fn nearest_palette_idx(c: Oklab, palette_lab: &[Oklab]) -> usize {
    let mut best_idx = 0;
    let mut best_d = f32::MAX;
    for (i, &p) in palette_lab.iter().enumerate() {
        let d = c.dist_sq(p);
        if d < best_d {
            best_d = d;
            best_idx = i;
        }
    }
    best_idx
}

pub fn apply_palette_and_dither(
    img: &RgbaImage,
    palette: &[[u8; 3]],
    mode: DitherMode,
) -> RgbaImage {
    let palette = if palette.is_empty() {
        &[[0, 0, 0]][..]
    } else {
        palette
    };
    let palette_lab: Vec<Oklab> = palette.iter().copied().map(srgb_u8_to_oklab).collect();

    let w = img.width();
    let h = img.height();
    let mut out = RgbaImage::new(w, h);

    match mode {
        DitherMode::None => {
            for (dst_p, src_p) in out.pixels_mut().zip(img.pixels()) {
                if src_p[3] < 128 {
                    *dst_p = Rgba([0, 0, 0, 0]);
                } else {
                    let lab = srgb_u8_to_oklab([src_p[0], src_p[1], src_p[2]]);
                    let rgb = palette[nearest_palette_idx(lab, &palette_lab)];
                    *dst_p = Rgba([rgb[0], rgb[1], rgb[2], 255]);
                }
            }
        }
        DitherMode::Bayer2x2 | DitherMode::Bayer4x4 => {
            let spread = 0.12 / (palette.len().max(2) as f32).cbrt();
            for y in 0..h {
                for x in 0..w {
                    let src_p = img.get_pixel(x, y);
                    if src_p[3] < 128 {
                        out.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                        continue;
                    }

                    let threshold = match mode {
                        DitherMode::Bayer2x2 => {
                            BAYER_2X2[(y & 1) as usize][(x & 1) as usize] / 4.0 - 0.375
                        }
                        _ => BAYER_4X4[(y & 3) as usize][(x & 3) as usize] / 16.0 - 0.46875,
                    };

                    let mut lab = srgb_u8_to_oklab([src_p[0], src_p[1], src_p[2]]);
                    lab.l += threshold * spread;
                    lab.a += threshold * spread * 0.5;
                    lab.b += threshold * spread * 0.5;

                    let rgb = palette[nearest_palette_idx(lab, &palette_lab)];
                    out.put_pixel(x, y, Rgba([rgb[0], rgb[1], rgb[2], 255]));
                }
            }
        }
        DitherMode::FloydSteinberg => {
            let total = (w * h) as usize;
            let mut labs: Vec<Oklab> = Vec::with_capacity(total);
            let mut opaque = Vec::with_capacity(total);

            for p in img.pixels() {
                let is_op = p[3] >= 128;
                opaque.push(is_op);
                if is_op {
                    labs.push(srgb_u8_to_oklab([p[0], p[1], p[2]]));
                } else {
                    labs.push(Oklab {
                        l: 0.0,
                        a: 0.0,
                        b: 0.0,
                    });
                }
            }

            let w_i = w as i32;
            let h_i = h as i32;

            for y in 0..h_i {
                for x in 0..w_i {
                    let idx = (y * w_i + x) as usize;
                    if !opaque[idx] {
                        out.put_pixel(x as u32, y as u32, Rgba([0, 0, 0, 0]));
                        continue;
                    }

                    let orig = labs[idx];
                    let pal_idx = nearest_palette_idx(orig, &palette_lab);
                    let chosen = palette_lab[pal_idx];
                    let rgb = palette[pal_idx];
                    out.put_pixel(x as u32, y as u32, Rgba([rgb[0], rgb[1], rgb[2], 255]));

                    let err_l = (orig.l - chosen.l).clamp(-0.25, 0.25);
                    let err_a = (orig.a - chosen.a).clamp(-0.25, 0.25);
                    let err_b = (orig.b - chosen.b).clamp(-0.25, 0.25);

                    let neighbors = [
                        (x + 1, y, 7.0 / 16.0),
                        (x - 1, y + 1, 3.0 / 16.0),
                        (x, y + 1, 5.0 / 16.0),
                        (x + 1, y + 1, 1.0 / 16.0),
                    ];

                    for (nx, ny, factor) in neighbors {
                        if nx >= 0 && nx < w_i && ny >= 0 && ny < h_i {
                            let n_idx = (ny * w_i + nx) as usize;
                            if opaque[n_idx] {
                                labs[n_idx].l += err_l * factor;
                                labs[n_idx].a += err_a * factor;
                                labs[n_idx].b += err_b * factor;
                            }
                        }
                    }
                }
            }
        }
    }

    out
}
