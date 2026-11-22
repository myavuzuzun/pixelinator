use image::{Rgba, RgbaImage};

use crate::pipeline::color::{oklab_to_srgb_u8, srgb_u8_to_oklab};

pub fn apply_postprocess(img: &RgbaImage, orphan_scrub: u8, outline: bool) -> RgbaImage {
    let mut out = match orphan_scrub {
        0 => img.clone(),
        1 => scrub_isolated_1px(img),
        2 => scrub_small_clusters(img, 2),
        _ => {
            let step1 = scrub_small_clusters(img, 2);
            let step2 = majority_block_pass(&step1);
            majority_block_pass(&step2)
        }
    };

    if outline {
        out = add_adaptive_outline(&out);
    }

    out
}

#[inline]
fn most_frequent_color(colors: &[[u8; 4]]) -> Option<[u8; 4]> {
    if colors.is_empty() {
        return None;
    }
    let mut best = colors[0];
    let mut best_count = 0usize;
    for &c in colors {
        let count = colors.iter().filter(|&&x| x == c).count();
        if count > best_count {
            best_count = count;
            best = c;
        }
    }
    Some(best)
}

fn scrub_isolated_1px(img: &RgbaImage) -> RgbaImage {
    let w = img.width() as i32;
    let h = img.height() as i32;
    let mut out = img.clone();

    for y in 0..h {
        for x in 0..w {
            let c = img.get_pixel(x as u32, y as u32).0;
            if c[3] < 128 {
                continue;
            }

            let mut has_match = false;
            let mut n4 = Vec::with_capacity(4);
            let mut n8 = Vec::with_capacity(8);

            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx < 0 || nx >= w || ny < 0 || ny >= h {
                        continue;
                    }
                    let nc = img.get_pixel(nx as u32, ny as u32).0;
                    if nc == c {
                        has_match = true;
                        break;
                    }
                    if nc[3] >= 128 {
                        n8.push(nc);
                        if dx == 0 || dy == 0 {
                            n4.push(nc);
                        }
                    }
                }
                if has_match {
                    break;
                }
            }

            if !has_match {
                if let Some(rep) = most_frequent_color(&n4).or_else(|| most_frequent_color(&n8)) {
                    out.put_pixel(x as u32, y as u32, Rgba(rep));
                }
            }
        }
    }

    out
}

fn scrub_small_clusters(img: &RgbaImage, max_cluster_size: usize) -> RgbaImage {
    let w = img.width() as usize;
    let h = img.height() as usize;
    let mut out = img.clone();
    let mut visited = vec![false; w * h];
    let mut queue: Vec<(usize, usize)> = Vec::new();
    let mut cluster: Vec<(usize, usize)> = Vec::new();
    let mut border_colors: Vec<[u8; 4]> = Vec::new();

    for sy in 0..h {
        for sx in 0..w {
            let s_idx = sy * w + sx;
            if visited[s_idx] {
                continue;
            }
            let c = img.get_pixel(sx as u32, sy as u32).0;
            if c[3] < 128 {
                visited[s_idx] = true;
                continue;
            }

            visited[s_idx] = true;
            queue.clear();
            cluster.clear();
            border_colors.clear();
            queue.push((sx, sy));

            let mut head = 0;
            while head < queue.len() {
                let (cx, cy) = queue[head];
                head += 1;
                cluster.push((cx, cy));

                let dirs: [(isize, isize); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
                for (dx, dy) in dirs {
                    let nx = cx as isize + dx;
                    let ny = cy as isize + dy;
                    if nx < 0 || nx >= w as isize || ny < 0 || ny >= h as isize {
                        continue;
                    }
                    let ux = nx as usize;
                    let uy = ny as usize;
                    let n_idx = uy * w + ux;
                    let nc = img.get_pixel(ux as u32, uy as u32).0;

                    if nc == c {
                        if !visited[n_idx] {
                            visited[n_idx] = true;
                            queue.push((ux, uy));
                        }
                    } else if nc[3] >= 128 {
                        border_colors.push(nc);
                    }
                }
            }

            if cluster.len() <= max_cluster_size {
                if let Some(rep) = most_frequent_color(&border_colors) {
                    for &(px, py) in &cluster {
                        out.put_pixel(px as u32, py as u32, Rgba(rep));
                    }
                }
            }
        }
    }

    out
}

fn majority_block_pass(img: &RgbaImage) -> RgbaImage {
    let w = img.width() as i32;
    let h = img.height() as i32;
    let mut out = img.clone();
    let mut neighbors = Vec::with_capacity(8);

    for y in 0..h {
        for x in 0..w {
            let c = img.get_pixel(x as u32, y as u32).0;
            if c[3] < 128 {
                continue;
            }

            neighbors.clear();
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx < 0 || nx >= w || ny < 0 || ny >= h {
                        continue;
                    }
                    let nc = img.get_pixel(nx as u32, ny as u32).0;
                    if nc[3] >= 128 {
                        neighbors.push(nc);
                    }
                }
            }

            if let Some(rep) = most_frequent_color(&neighbors) {
                let count = neighbors.iter().filter(|&&n| n == rep).count();
                if rep != c && count >= 5 {
                    out.put_pixel(x as u32, y as u32, Rgba(rep));
                }
            }
        }
    }

    out
}

fn add_adaptive_outline(img: &RgbaImage) -> RgbaImage {
    let w = img.width() as i32;
    let h = img.height() as i32;
    let mut out = img.clone();

    let dirs: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

    for y in 0..h {
        for x in 0..w {
            let p = img.get_pixel(x as u32, y as u32).0;
            if p[3] > 0 {
                continue;
            }

            let mut darkest_rgb: Option<[u8; 3]> = None;
            let mut darkest_l = f32::MAX;

            for (dx, dy) in dirs {
                let nx = x + dx;
                let ny = y + dy;
                if nx < 0 || nx >= w || ny < 0 || ny >= h {
                    continue;
                }
                let np = img.get_pixel(nx as u32, ny as u32).0;
                if np[3] > 0 {
                    let lab = srgb_u8_to_oklab([np[0], np[1], np[2]]);
                    if lab.l < darkest_l {
                        darkest_l = lab.l;
                        darkest_rgb = Some([np[0], np[1], np[2]]);
                    }
                }
            }

            if let Some(rgb) = darkest_rgb {
                let mut lab = srgb_u8_to_oklab(rgb);
                lab.l = (lab.l * 0.28).clamp(0.05, 0.32);
                lab.a *= 0.6;
                lab.b *= 0.6;
                let dark = oklab_to_srgb_u8(lab);
                out.put_pixel(x as u32, y as u32, Rgba([dark[0], dark[1], dark[2], 255]));
            }
        }
    }

    out
}
