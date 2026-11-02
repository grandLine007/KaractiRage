//! Capture et diff de prévisualisation — fondation de `ai_review`.
//!
//! Une capture réduit un framebuffer `0x00RRGGBB` à une signature en niveaux de gris
//! 8×8 (déterministe et peu coûteuse). `preview_diff` mesure la différence moyenne
//! absolue entre deux captures (0.0 = identique, 1.0 = opposée).

/// Taille de la grille de la signature (8×8).
pub const PREVIEW_GRID: usize = 8;

/// Signature de prévisualisation downsampled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewCapture {
    pub width: usize,
    pub height: usize,
    /// Niveaux de gris 0..255 sur une grille `PREVIEW_GRID × PREVIEW_GRID`.
    pub signature: [u8; PREVIEW_GRID * PREVIEW_GRID],
}

/// Capture une signature déterministe d'un framebuffer `0x00RRGGBB`.
pub fn capture_preview(buf: &[u32], width: usize, height: usize) -> PreviewCapture {
    let mut signature = [0u8; PREVIEW_GRID * PREVIEW_GRID];
    if width == 0 || height == 0 || buf.len() < width * height {
        return PreviewCapture {
            width,
            height,
            signature,
        };
    }

    for gy in 0..PREVIEW_GRID {
        for gx in 0..PREVIEW_GRID {
            let x0 = gx * width / PREVIEW_GRID;
            let x1 = ((gx + 1) * width / PREVIEW_GRID).max(x0 + 1);
            let y0 = gy * height / PREVIEW_GRID;
            let y1 = ((gy + 1) * height / PREVIEW_GRID).max(y0 + 1);

            let mut sum = 0u64;
            let mut count = 0u64;
            for y in y0..y1.min(height) {
                for x in x0..x1.min(width) {
                    let p = buf[y * width + x];
                    let r = ((p >> 16) & 0xFF) as u64;
                    let g = ((p >> 8) & 0xFF) as u64;
                    let b = (p & 0xFF) as u64;
                    // Pondération de luminance (Rec. 601).
                    sum += (r * 299 + g * 587 + b * 114) / 1000;
                    count += 1;
                }
            }
            signature[gy * PREVIEW_GRID + gx] = if count == 0 {
                0
            } else {
                (sum / count) as u8
            };
        }
    }

    PreviewCapture {
        width,
        height,
        signature,
    }
}

/// Différence moyenne absolue entre deux captures (0.0 = identique, 1.0 = opposée).
pub fn preview_diff(a: &PreviewCapture, b: &PreviewCapture) -> f32 {
    let mut sum = 0u32;
    for (x, y) in a.signature.iter().zip(b.signature.iter()) {
        sum += (*x as i32 - *y as i32).unsigned_abs();
    }
    sum as f32 / (PREVIEW_GRID * PREVIEW_GRID) as f32 / 255.0
}

/// Verdict d'une revue de prévisualisation (`ai_review`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewVerdict {
    /// Différence négligeable (capture quasi identique).
    NoChange,
    /// Changement modéré.
    Minor,
    /// Changement important.
    Major,
}

/// Rapport de revue de prévisualisation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReviewReport {
    pub diff: f32,
    pub verdict: ReviewVerdict,
}

/// Compare deux captures et produit un rapport de revue avec seuils.
pub fn review_preview(before: &PreviewCapture, after: &PreviewCapture) -> ReviewReport {
    let diff = preview_diff(before, after);
    let verdict = if diff < 0.01 {
        ReviewVerdict::NoChange
    } else if diff < 0.15 {
        ReviewVerdict::Minor
    } else {
        ReviewVerdict::Major
    };
    ReviewReport { diff, verdict }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_is_deterministic_and_self_diff_is_zero() {
        let w = 64;
        let h = 64;
        let buf: Vec<u32> = (0..(w * h)).map(|i| 0x00_000000_u32 | ((i % 255) as u32)).collect();

        let a = capture_preview(&buf, w, h);
        let b = capture_preview(&buf, w, h);
        assert_eq!(a, b);
        assert_eq!(preview_diff(&a, &b), 0.0);
        assert_eq!(a.width, w);
        assert_eq!(a.height, h);
    }

    #[test]
    fn diff_detects_change_and_clamps_to_unit_interval() {
        let w = 64;
        let h = 64;
        let black: Vec<u32> = vec![0x00_000000; w * h];
        let white: Vec<u32> = vec![0x00_FFFFFF; w * h];

        let a = capture_preview(&black, w, h);
        let b = capture_preview(&white, w, h);

        let d = preview_diff(&a, &b);
        assert!(d > 0.9, "noir vs blanc doit différer fortement (d = {d})");
        assert!(d <= 1.0);
    }

    #[test]
    fn empty_buffer_is_handled_without_panic() {
        let c = capture_preview(&[], 0, 0);
        assert_eq!(c.signature, [0u8; PREVIEW_GRID * PREVIEW_GRID]);
        assert_eq!(preview_diff(&c, &c), 0.0);
    }

    #[test]
    fn review_preview_classifies_by_thresholds() {
        let w = 64;
        let h = 64;
        let black: Vec<u32> = vec![0x00_000000; w * h];
        let white: Vec<u32> = vec![0x00_FFFFFF; w * h];
        let gray: Vec<u32> = vec![0x00_808080; w * h];

        let b = capture_preview(&black, w, h);
        let wcap = capture_preview(&white, w, h);
        let g = capture_preview(&gray, w, h);

        assert_eq!(review_preview(&b, &b).verdict, ReviewVerdict::NoChange);
        assert_eq!(review_preview(&b, &g).verdict, ReviewVerdict::Major);
        assert_eq!(review_preview(&b, &wcap).verdict, ReviewVerdict::Major);
    }
}
