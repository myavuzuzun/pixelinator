use image::{Rgba, RgbaImage};

struct Bucket {
    key: u16,
    count: u32,
    r_sum: u32,
    g_sum: u32,
    b_sum: u32,
    min_center_dist: u32,
}

/// Crisp, edge-aware downscaling using modal color-bucket selection per cell.
pub fn downscale_edge_aware(src: &RgbaImage, target_w: u32, target_h: u32) -> RgbaImage {
    let tw = target_w.max(1);
    let th = target_h.max(1);
    let sw = src.width().max(1);
    let sh = src.height().max(1);

    let mut dst = RgbaImage::new(tw, th);
    let s_raw = src.as_raw();
    let d_raw = dst.as_mut();

    if tw >= sw && th >= sh {
        for oy in 0..th {
            let sy = ((oy as u64 * sh as u64) / th as u64).min(sh as u64 - 1) as usize;
            for ox in 0..tw {
                let sx = ((ox as u64 * sw as u64) / tw as u64).min(sw as u64 - 1) as usize;
                let sp = (sy * sw as usize + sx) * 4;
                let dp = (oy as usize * tw as usize + ox as usize) * 4;
                d_raw[dp..dp + 4].copy_from_slice(&s_raw[sp..sp + 4]);
            }
        }
        return dst;
    }

    let mut buckets: Vec<Bucket> = Vec::with_capacity(32);

    for oy in 0..th {
        let y0 = ((oy as u64 * sh as u64) / th as u64) as u32;
        let y1 = (((oy + 1) as u64 * sh as u64) / th as u64)
            .max(y0 as u64 + 1)
            .min(sh as u64) as u32;
        let cy = (y0 + y1) as i32;
        let y_span = y1 - y0;
        let y_step = (y_span / 8).max(1);

        for ox in 0..tw {
            let x0 = ((ox as u64 * sw as u64) / tw as u64) as u32;
            let x1 = (((ox + 1) as u64 * sw as u64) / tw as u64)
                .max(x0 as u64 + 1)
                .min(sw as u64) as u32;
            let cx = (x0 + x1) as i32;
            let x_span = x1 - x0;
            let x_step = (x_span / 8).max(1);

            buckets.clear();
            let mut opaque_count = 0u32;
            let mut transparent_count = 0u32;

            let mut sy = y0;
            while sy < y1 {
                let mut sx = x0;
                while sx < x1 {
                    let idx = ((sy * sw + sx) * 4) as usize;
                    let a = s_raw[idx + 3];
                    if a < 128 {
                        transparent_count += 1;
                    } else {
                        opaque_count += 1;
                        let r = s_raw[idx];
                        let g = s_raw[idx + 1];
                        let b = s_raw[idx + 2];
                        let key =
                            (((r >> 5) as u16) << 6) | (((g >> 5) as u16) << 3) | ((b >> 5) as u16);

                        let dx = sx as i32 * 2 + 1 - cx;
                        let dy = sy as i32 * 2 + 1 - cy;
                        let dist = (dx * dx + dy * dy) as u32;

                        if let Some(bucket) = buckets.iter_mut().find(|bk| bk.key == key) {
                            bucket.count += 1;
                            bucket.r_sum += r as u32;
                            bucket.g_sum += g as u32;
                            bucket.b_sum += b as u32;
                            if dist < bucket.min_center_dist {
                                bucket.min_center_dist = dist;
                            }
                        } else {
                            buckets.push(Bucket {
                                key,
                                count: 1,
                                r_sum: r as u32,
                                g_sum: g as u32,
                                b_sum: b as u32,
                                min_center_dist: dist,
                            });
                        }
                    }
                    sx += x_step;
                }
                sy += y_step;
            }

            let out_pixel = if transparent_count > opaque_count || buckets.is_empty() {
                Rgba([0, 0, 0, 0])
            } else {
                let best = buckets
                    .iter()
                    .max_by(|a, b| {
                        a.count
                            .cmp(&b.count)
                            .then_with(|| b.min_center_dist.cmp(&a.min_center_dist))
                    })
                    .unwrap();
                let n = best.count;
                Rgba([
                    (best.r_sum / n) as u8,
                    (best.g_sum / n) as u8,
                    (best.b_sum / n) as u8,
                    255,
                ])
            };

            dst.put_pixel(ox, oy, out_pixel);
        }
    }

    dst
}
