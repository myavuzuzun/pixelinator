use std::collections::HashMap;
use std::fs;
use std::path::Path;

use image::RgbaImage;

#[derive(Clone, Copy, Debug)]
pub struct Oklab {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

impl Oklab {
    #[inline]
    pub fn dist_sq(self, other: Self) -> f32 {
        let dl = self.l - other.l;
        let da = self.a - other.a;
        let db = self.b - other.b;
        dl * dl + da * da + db * db
    }
}

#[inline]
fn srgb_to_linear(c: u8) -> f32 {
    let x = c as f32 / 255.0;
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
fn linear_to_srgb_u8(x: f32) -> u8 {
    let clamped = x.clamp(0.0, 1.0);
    let s = if clamped <= 0.0031308 {
        clamped * 12.92
    } else {
        1.055 * clamped.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

#[inline]
pub fn srgb_u8_to_oklab(rgb: [u8; 3]) -> Oklab {
    let r = srgb_to_linear(rgb[0]);
    let g = srgb_to_linear(rgb[1]);
    let b = srgb_to_linear(rgb[2]);

    let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
    let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
    let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    Oklab {
        l: 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        a: 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        b: 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
    }
}

#[inline]
pub fn oklab_to_srgb_u8(c: Oklab) -> [u8; 3] {
    let l_ = c.l + 0.3963377774 * c.a + 0.2158037573 * c.b;
    let m_ = c.l - 0.1055613458 * c.a - 0.0638541728 * c.b;
    let s_ = c.l - 0.0894841775 * c.a - 1.2914855480 * c.b;

    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;

    let r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
    let g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
    let b = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s;

    [
        linear_to_srgb_u8(r),
        linear_to_srgb_u8(g),
        linear_to_srgb_u8(b),
    ]
}

struct Xorshift64(u64);

impl Xorshift64 {
    fn next_f32(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Deterministic weighted k-means++ in Oklab space over opaque pixels.
pub fn extract_auto_palette(img: &RgbaImage, k: usize) -> Vec<[u8; 3]> {
    let k = k.clamp(2, 64);
    let mut freq: HashMap<[u8; 3], u32> = HashMap::new();

    for p in img.pixels() {
        if p[3] >= 128 {
            *freq.entry([p[0], p[1], p[2]]).or_insert(0) += 1;
        }
    }

    if freq.is_empty() {
        return vec![[0, 0, 0]];
    }

    if freq.len() <= k {
        let mut colors: Vec<[u8; 3]> = freq.into_keys().collect();
        colors.sort_unstable();
        return colors;
    }

    // Sort entries deterministically so k-means++ is 100% reproducible.
    let mut entries: Vec<([u8; 3], u32)> = freq.into_iter().collect();
    entries.sort_unstable_by_key(|(rgb, _)| *rgb);

    let samples: Vec<(Oklab, f32)> = entries
        .into_iter()
        .map(|(rgb, count)| (srgb_u8_to_oklab(rgb), count as f32))
        .collect();

    let mut centroids: Vec<Oklab> = Vec::with_capacity(k);
    // Pick highest-weight color as first centroid.
    let first_idx = samples
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
        .unwrap_or(0);
    centroids.push(samples[first_idx].0);

    let mut min_dist_sq: Vec<f32> = samples
        .iter()
        .map(|(lab, _)| lab.dist_sq(centroids[0]))
        .collect();

    let mut rng = Xorshift64(0x9E37_79B9_7F4A_7C15);

    while centroids.len() < k {
        let mut total_weight = 0.0f64;
        for (i, (_, w)) in samples.iter().enumerate() {
            total_weight += (min_dist_sq[i] as f64) * (*w as f64);
        }

        let next_idx = if total_weight <= 0.0 {
            centroids.len() % samples.len()
        } else {
            let target = (rng.next_f32() as f64) * total_weight;
            let mut acc = 0.0f64;
            let mut chosen = samples.len() - 1;
            for (i, (_, w)) in samples.iter().enumerate() {
                acc += (min_dist_sq[i] as f64) * (*w as f64);
                if acc >= target {
                    chosen = i;
                    break;
                }
            }
            chosen
        };

        let c = samples[next_idx].0;
        centroids.push(c);

        for (i, (lab, _)) in samples.iter().enumerate() {
            let d = lab.dist_sq(c);
            if d < min_dist_sq[i] {
                min_dist_sq[i] = d;
            }
        }
    }

    let mut sums_l = vec![0.0f64; k];
    let mut sums_a = vec![0.0f64; k];
    let mut sums_b = vec![0.0f64; k];
    let mut weights = vec![0.0f64; k];

    for _ in 0..12 {
        sums_l.fill(0.0);
        sums_a.fill(0.0);
        sums_b.fill(0.0);
        weights.fill(0.0);

        for &(lab, w) in &samples {
            let mut best_idx = 0;
            let mut best_d = f32::MAX;
            for (ci, &c) in centroids.iter().enumerate() {
                let d = lab.dist_sq(c);
                if d < best_d {
                    best_d = d;
                    best_idx = ci;
                }
            }
            let wf = w as f64;
            sums_l[best_idx] += (lab.l as f64) * wf;
            sums_a[best_idx] += (lab.a as f64) * wf;
            sums_b[best_idx] += (lab.b as f64) * wf;
            weights[best_idx] += wf;
        }

        for ci in 0..k {
            if weights[ci] > 0.0 {
                centroids[ci] = Oklab {
                    l: (sums_l[ci] / weights[ci]) as f32,
                    a: (sums_a[ci] / weights[ci]) as f32,
                    b: (sums_b[ci] / weights[ci]) as f32,
                };
            }
        }
    }

    centroids.into_iter().map(oklab_to_srgb_u8).collect()
}

pub fn load_palette_file(path: &Path) -> Result<Vec<[u8; 3]>, String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("failed to read palette file: {e}"))?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    let colors = match ext.as_str() {
        "hex" => parse_hex(&content),
        "pal" => parse_pal(&content),
        "gpl" => parse_gpl(&content),
        _ => return Err("unsupported palette format (use .hex, .pal, or .gpl)".to_string()),
    };

    if colors.is_empty() {
        return Err("no valid colors found in palette file".to_string());
    }

    Ok(colors)
}

fn parse_hex(content: &str) -> Vec<[u8; 3]> {
    let mut colors = Vec::new();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') && line.len() == 1 || line.starts_with("//") {
            continue;
        }
        let token = line
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches('#')
            .trim_start_matches("0x")
            .trim_start_matches("0X");

        if token.len() == 6 || token.len() == 8 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&token[0..2], 16),
                u8::from_str_radix(&token[2..4], 16),
                u8::from_str_radix(&token[4..6], 16),
            ) {
                colors.push([r, g, b]);
            }
        }
    }
    colors
}

fn parse_pal(content: &str) -> Vec<[u8; 3]> {
    let mut colors = Vec::new();
    let mut lines = content.lines().map(str::trim).filter(|l| !l.is_empty());

    let mut is_jasc = false;
    if let Some(first) = lines.next() {
        if first.eq_ignore_ascii_case("JASC-PAL") {
            is_jasc = true;
            // Skip version and count lines.
            let _ = lines.next();
            let _ = lines.next();
        } else if let Some(rgb) = parse_rgb_triplet(first) {
            colors.push(rgb);
        }
    }

    for line in lines {
        if is_jasc && line.starts_with('#') {
            continue;
        }
        if let Some(rgb) = parse_rgb_triplet(line) {
            colors.push(rgb);
        }
    }
    colors
}

fn parse_gpl(content: &str) -> Vec<[u8; 3]> {
    let mut colors = Vec::new();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with("GIMP Palette")
            || line.starts_with("Name:")
            || line.starts_with("Columns:")
        {
            continue;
        }
        if let Some(rgb) = parse_rgb_triplet(line) {
            colors.push(rgb);
        }
    }
    colors
}

fn parse_rgb_triplet(line: &str) -> Option<[u8; 3]> {
    let mut parts = line.split_whitespace();
    let r = parts.next()?.parse::<u16>().ok()?;
    let g = parts.next()?.parse::<u16>().ok()?;
    let b = parts.next()?.parse::<u16>().ok()?;
    if r <= 255 && g <= 255 && b <= 255 {
        Some([r as u8, g as u8, b as u8])
    } else {
        None
    }
}
