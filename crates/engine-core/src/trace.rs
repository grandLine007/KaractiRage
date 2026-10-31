//! Vectorisation raster → SVG (quantification median-cut + marching squares +
//! simplification Douglas–Peucker). Aucune dépendance externe : le bitmap source
//! vient de `crate::vector::RasterImage` (prémultiplié `0xAARRGGBB`, 0 = transparent).

use serde::{Deserialize, Serialize};

use crate::vector::RasterImage;

/// Paramètres de vectorisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceOptions {
    /// Nombre de couleurs de la palette (2..=16).
    pub colors: usize,
    /// Seuil d'alpha : un pixel est opaque si alpha > seuil.
    pub alpha_threshold: u8,
    /// Dimension max avant tracé (0 = pas de réduction). Préserve le look pixel-art.
    pub downscale_max: u32,
    /// Epsilon Douglas–Peucker en pixels (0 = pixel-perfect, escaliers conservés).
    pub simplify: f32,
}

impl Default for TraceOptions {
    fn default() -> Self {
        Self { colors: 8, alpha_threshold: 32, downscale_max: 256, simplify: 1.0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorPath {
    pub points: Vec<(f32, f32)>,
    #[serde(default = "default_true")]
    pub closed: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorLayer {
    /// Couleur hex `#rrggbb`.
    pub color: String,
    pub paths: Vec<VectorPath>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorModel {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<VectorLayer>,
}

impl VectorModel {
    pub fn to_svg(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
            self.width, self.height, self.width, self.height
        ));
        for layer in &self.layers {
            if layer.paths.is_empty() {
                continue;
            }
            // Les sous-chemins d'une couche sont combinés dans un seul <path>
            // pour que `fill-rule="evenodd"` creuse correctement les trous.
            let mut d = String::new();
            for path in &layer.paths {
                if path.points.len() < 2 {
                    continue;
                }
                d.push_str(&format!("M {} {}", path.points[0].0, path.points[0].1));
                for (x, y) in &path.points[1..] {
                    d.push_str(&format!(" L {} {}", x, y));
                }
                if path.closed {
                    d.push_str(" Z ");
                }
            }
            s.push_str(&format!("  <path fill=\"{}\" fill-rule=\"evenodd\" d=\"{}\"/>\n", layer.color, d.trim()));
        }
        s.push_str("</svg>\n");
        s
    }
}

/// Canal RGB d'un pixel par index (0 = R, 1 = G, 2 = B).
fn channel(p: &(u8, u8, u8), ch: usize) -> u8 {
    match ch {
        0 => p.0,
        1 => p.1,
        _ => p.2,
    }
}

/// Quantification median-cut : réduit un nuage RGB à `colors` couleurs moyennes.
fn median_cut(pixels: &[(u8, u8, u8)], colors: usize) -> Vec<(u8, u8, u8)> {
    let mut boxes: Vec<Vec<(u8, u8, u8)>> = vec![pixels.to_vec()];
    while boxes.len() < colors {
        let (mut best, mut best_range, mut best_ch) = (0usize, 0u8, 0usize);
        for (i, b) in boxes.iter().enumerate() {
            if b.len() < 2 {
                continue;
            }
            for ch in 0..3 {
                let mn = b.iter().map(|p| channel(p, ch)).min().unwrap();
                let mx = b.iter().map(|p| channel(p, ch)).max().unwrap();
                if mx - mn > best_range {
                    best_range = mx - mn;
                    best = i;
                    best_ch = ch;
                }
            }
        }
        if best_range == 0 {
            break;
        }
        let mut b = boxes[best].clone();
        b.sort_by_key(|p| channel(p, best_ch));
        let mid = b.len() / 2;
        boxes[best] = b[..mid].to_vec();
        boxes.insert(best + 1, b[mid..].to_vec());
    }
    boxes
        .into_iter()
        .map(|b| {
            if b.is_empty() {
                (0u8, 0u8, 0u8)
            } else {
                let n = b.len() as u64;
                let (sr, sg, sb) = b
                    .iter()
                    .fold((0u64, 0u64, 0u64), |a, p| (a.0 + p.0 as u64, a.1 + p.1 as u64, a.2 + p.2 as u64));
                ((sr / n) as u8, (sg / n) as u8, (sb / n) as u8)
            }
        })
        .collect()
}

/// Indice de la couleur de palette la plus proche d'un pixel RGB.
fn nearest(palette: &[(u8, u8, u8)], rgb: (u8, u8, u8)) -> usize {
    let mut best = 0;
    let mut bd = u32::MAX;
    for (i, p) in palette.iter().enumerate() {
        let dr = p.0 as i32 - rgb.0 as i32;
        let dg = p.1 as i32 - rgb.1 as i32;
        let db = p.2 as i32 - rgb.2 as i32;
        let d = (dr * dr + dg * dg + db * db) as u32;
        if d < bd {
            bd = d;
            best = i;
        }
    }
    best
}

/// Réduction au plus proche voisin (conserve les bords nets du pixel-art).
fn downscale(img: &RasterImage, max_dim: u32) -> (u32, u32, Vec<u32>) {
    let (tw, th) = if img.width >= img.height {
        (max_dim, ((img.height as f32 * max_dim as f32 / img.width as f32).round() as u32).max(1))
    } else {
        (((img.width as f32 * max_dim as f32 / img.height as f32).round() as u32).max(1), max_dim)
    };
    let mut out = vec![0u32; (tw * th) as usize];
    for ty in 0..th {
        for tx in 0..tw {
            let sx = (tx * img.width / tw).min(img.width - 1);
            let sy = (ty * img.height / th).min(img.height - 1);
            out[(ty * tw + tx) as usize] = img.pixels[(sy * img.width + sx) as usize];
        }
    }
    (tw, th, out)
}
fn close(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < 0.001 && (a.1 - b.1).abs() < 0.001
}

/// Marching squares : extrait les contours fermés d'un masque binaire.
fn trace_mask(w: usize, h: usize, filled: &[bool]) -> Vec<Vec<(f32, f32)>> {
    let mut segs: Vec<((f32, f32), (f32, f32))> = vec![];
    for y in 0..h.saturating_sub(1) {
        for x in 0..w.saturating_sub(1) {
            let tl = filled[y * w + x];
            let tr = filled[y * w + x + 1];
            let br = filled[(y + 1) * w + x + 1];
            let bl = filled[(y + 1) * w + x];
            let case = (tl as u8) | ((tr as u8) << 1) | ((br as u8) << 2) | ((bl as u8) << 3);
            let top = (x as f32 + 0.5, y as f32);
            let right = (x as f32 + 1.0, y as f32 + 0.5);
            let bottom = (x as f32 + 0.5, y as f32 + 1.0);
            let left = (x as f32, y as f32 + 0.5);
            let edges = [top, right, bottom, left];
            let pairs: &[(usize, usize)] = match case {
                1 => &[(0, 3)],
                2 => &[(0, 1)],
                3 => &[(3, 1)],
                4 => &[(1, 2)],
                5 => &[(0, 3), (1, 2)],
                6 => &[(0, 2)],
                7 => &[(3, 2)],
                8 => &[(2, 3)],
                9 => &[(0, 2)],
                10 => &[(0, 1), (2, 3)],
                11 => &[(1, 2)],
                12 => &[(1, 3)],
                13 => &[(0, 1)],
                14 => &[(0, 3)],
                _ => &[],
            };
            for &(a, b) in pairs {
                segs.push((edges[a], edges[b]));
            }
        }
    }

    let mut used = vec![false; segs.len()];
    let mut loops: Vec<Vec<(f32, f32)>> = vec![];
    for i in 0..segs.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let mut pts = vec![segs[i].0, segs[i].1];
        let mut cur = segs[i].1;
        loop {
            let mut next: Option<usize> = None;
            let mut flip = false;
            for j in 0..segs.len() {
                if used[j] {
                    continue;
                }
                if close(segs[j].0, cur) {
                    next = Some(j);
                    flip = false;
                    break;
                }
                if close(segs[j].1, cur) {
                    next = Some(j);
                    flip = true;
                    break;
                }
            }
            match next {
                Some(j) => {
                    used[j] = true;
                    cur = if flip { segs[j].0 } else { segs[j].1 };
                    pts.push(cur);
                }
                None => break,
            }
        }
        let mut clean: Vec<(f32, f32)> = vec![];
        for p in pts {
            if clean.last().map(|q| !close(*q, p)).unwrap_or(true) {
                clean.push(p);
            }
        }
        if clean.len() > 1 && close(clean[0], clean[clean.len() - 1]) {
            clean.pop();
        }
        if clean.len() >= 3 {
            loops.push(clean);
        }
    }
    loops
}
fn pt_seg_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        let (ex, ey) = (p.0 - a.0, p.1 - a.1);
        return (ex * ex + ey * ey).sqrt();
    }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0);
    let (cx, cy) = (a.0 + t * dx, a.1 + t * dy);
    let (ex, ey) = (p.0 - cx, p.1 - cy);
    (ex * ex + ey * ey).sqrt()
}

fn dp(points: &[(f32, f32)], i: usize, j: usize, eps: f32, out: &mut Vec<(f32, f32)>) {
    if j <= i + 1 {
        return;
    }
    let (mut maxd, mut idx) = (0.0f32, i + 1);
    for k in i + 1..j {
        let d = pt_seg_dist(points[k], points[i], points[j]);
        if d > maxd {
            maxd = d;
            idx = k;
        }
    }
    if maxd > eps {
        dp(points, i, idx, eps, out);
        out.push(points[idx]);
        dp(points, idx, j, eps, out);
    }
}

/// Simplification Douglas–Peucker (eps ≤ 0 → aucun lissage, escaliers conservés),
/// suivie d'une passe qui retire les points colinéaires consécutifs.
fn simplify(points: &[(f32, f32)], eps: f32) -> Vec<(f32, f32)> {
    let pts = if points.len() <= 2 || eps <= 0.0 {
        points.to_vec()
    } else {
        let mut out = vec![points[0]];
        dp(points, 0, points.len() - 1, eps, &mut out);
        out.push(points[points.len() - 1]);
        out
    };
    let mut res: Vec<(f32, f32)> = vec![];
    for p in pts {
        let n = res.len();
        if n >= 2 && collinear(res[n - 2], res[n - 1], p, 0.01) {
            res[n - 1] = p;
        } else {
            res.push(p);
        }
    }
    res
}

fn collinear(a: (f32, f32), b: (f32, f32), c: (f32, f32), eps: f32) -> bool {
    ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)).abs() < eps
}

/// Vectorise un raster en `VectorModel` (couches colorées → chemins fermés).
pub fn vectorize_raster(img: &RasterImage, opts: &TraceOptions) -> Result<VectorModel, String> {
    let (w, h, pixels) = if opts.downscale_max > 0 && (img.width > opts.downscale_max || img.height > opts.downscale_max) {
        downscale(img, opts.downscale_max)
    } else {
        (img.width, img.height, img.pixels.clone())
    };

    let mut opaque: Vec<(u8, u8, u8)> = vec![];
    for &p in &pixels {
        if ((p >> 24) & 0xFF) > opts.alpha_threshold as u32 {
            opaque.push((((p >> 16) & 0xFF) as u8, ((p >> 8) & 0xFF) as u8, (p & 0xFF) as u8));
        }
    }
    if opaque.is_empty() {
        return Err("Image entièrement transparente (seuil alpha trop haut ?)".into());
    }

    let colors = opts.colors.clamp(2, 16);
    let palette = median_cut(&opaque, colors);

    let mut idx: Vec<u8> = vec![255; (w * h) as usize];
    for i in 0..(w * h) as usize {
        let p = pixels[i];
        if ((p >> 24) & 0xFF) > opts.alpha_threshold as u32 {
            let rgb = (((p >> 16) & 0xFF) as u8, ((p >> 8) & 0xFF) as u8, (p & 0xFF) as u8);
            idx[i] = nearest(&palette, rgb) as u8;
        }
    }

    let mut layers: Vec<VectorLayer> = vec![];
    for (ci, &(r, g, b)) in palette.iter().enumerate() {
        let mut filled = vec![false; (w * h) as usize];
        for (i, f) in filled.iter_mut().enumerate() {
            *f = idx[i] == ci as u8;
        }
        let loops = trace_mask(w as usize, h as usize, &filled);
        if loops.is_empty() {
            continue;
        }
        let color = format!("#{r:02x}{g:02x}{b:02x}");
        let paths = loops
            .into_iter()
            .filter_map(|lp| {
                let pts = simplify(&lp, opts.simplify);
                if pts.len() < 3 {
                    None
                } else {
                    Some(VectorPath { points: pts, closed: true })
                }
            })
            .collect::<Vec<_>>();
        if !paths.is_empty() {
            layers.push(VectorLayer { color, paths });
        }
    }

    Ok(VectorModel { width: w, height: h, layers })
}
#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: u32, h: u32, fill: impl Fn(u32, u32) -> bool, color: u32) -> RasterImage {
        let mut pixels = vec![0u32; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                if fill(x, y) {
                    pixels[(y * w + x) as usize] = color;
                }
            }
        }
        RasterImage { width: w, height: h, pixels }
    }

    #[test]
    fn solid_square_is_one_loop() {
        let im = img(16, 16, |x, y| (2..=13).contains(&x) && (2..=13).contains(&y), 0xFF_FF_00_00);
        let model = vectorize_raster(&im, &TraceOptions { colors: 2, ..Default::default() }).unwrap();
        assert_eq!(model.layers.len(), 1);
        assert_eq!(model.layers[0].paths.len(), 1);
        assert!(model.layers[0].paths[0].closed);
        // Rectangle : 4 coins, +1 point de départ éventuel au milieu d'une arête
        // (le segment de fermeture `Z` complète l'arête). ≤ 6 points acceptés.
        assert!(model.layers[0].paths[0].points.len() >= 4 && model.layers[0].paths[0].points.len() <= 6);
    }

    #[test]
    fn donut_is_two_loops() {
        let im = img(16, 16, |x, y| {
            let outer = (1..=14).contains(&x) && (1..=14).contains(&y);
            let hole = (6..=9).contains(&x) && (6..=9).contains(&y);
            outer && !hole
        }, 0xFF_00_FF_00);
        let model = vectorize_raster(&im, &TraceOptions { colors: 2, simplify: 0.0, ..Default::default() }).unwrap();
        assert_eq!(model.layers.len(), 1);
        assert_eq!(model.layers[0].paths.len(), 2);
    }

    #[test]
    fn svg_is_well_formed() {
        let im = img(8, 8, |x, y| (1..=6).contains(&x) && (1..=6).contains(&y), 0xFF_00_00_FF);
        let model = vectorize_raster(&im, &TraceOptions::default()).unwrap();
        let svg = model.to_svg();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("<path"));
        assert!(svg.ends_with("</svg>\n"));
    }
}