use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Division entière exacte par 255 pour `v` dans [0, 65025].
///
/// Identité : `v / 255 == (v + 1 + (v >> 8)) >> 8` pour tout `v <= 65025`.
/// Évite 3 divisions matérielles par pixel de glyphe dans le blending alpha.
#[inline]
fn div255(v: u32) -> u32 {
    (v + 1 + (v >> 8)) >> 8
}

/// Glyphe pré-rastérisé mis en cache pour éliminer toute allocation heap dans le hot path de rendu
#[derive(Clone)]
pub struct CachedGlyph {
    pub metrics: fontdue::Metrics,
    pub bitmap: Arc<[u8]>,
}

/// Cache global de glyphes indexé par (pointeur_police, caractère, taille_quantifiée_x10)
pub static GLYPH_CACHE: RwLock<Option<HashMap<(usize, char, u16), CachedGlyph>>> = RwLock::new(None);

/// Accède à un glyphe depuis le cache sans aucune allocation heap en régime de croisière
#[inline]
pub fn with_cached_glyph<R>(
    font: &fontdue::Font,
    c: char,
    size: f32,
    f: impl FnOnce(&fontdue::Metrics, &[u8]) -> R,
) -> R {
    let font_ptr = font as *const fontdue::Font as usize;
    let size_key = (size * 10.0).round() as u16;
    let key = (font_ptr, c, size_key);

    // 1. Fast Path : Accès immédiat en lecture sans clonage d'Arc ni allocation
    if let Ok(guard) = GLYPH_CACHE.read() {
        if let Some(ref cache) = *guard {
            if let Some(cached) = cache.get(&key) {
                return f(&cached.metrics, &cached.bitmap);
            }
        }
    }

    // 2. Slow Path : Rastérisation unique et mise en cache
    let (metrics, bitmap) = font.rasterize(c, size);
    let bitmap_arc: Arc<[u8]> = Arc::from(bitmap.into_boxed_slice());
    let cached = CachedGlyph {
        metrics,
        bitmap: bitmap_arc.clone(),
    };

    if let Ok(mut guard) = GLYPH_CACHE.write() {
        let cache = guard.get_or_insert_with(|| HashMap::with_capacity(512));
        cache.insert(key, cached);
    }

    f(&metrics, &bitmap_arc)
}

/// Préchauffe le cache de glyphes pour les polices et tailles clés de l'interface
pub fn warmup_glyph_cache() {
    let title = get_title_font();
    let body = get_body_font();
    let mono = get_mono_font();
    let fonts = [title, body, mono];
    let sizes = [10.0, 11.0, 12.0, 13.0, 14.0, 16.0, 17.0, 20.0];
    let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.,:;!?+-*/=()[]{}<>'\"_#%~@$éèêëàâîïôùûçÉÈÀÇ°«»€ ";

    for &font in &fonts {
        for &size in &sizes {
            for c in chars.chars() {
                with_cached_glyph(font, c, size, |_, _| {});
            }
        }
    }
}

pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub buffer: Vec<u32>,
    pub clip_rect: Option<(i32, i32, u32, u32)>,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            buffer: vec![0; width * height],
            clip_rect: None,
        }
    }

    pub fn set_clip_rect(&mut self, rect: Option<(i32, i32, u32, u32)>) {
        self.clip_rect = rect;
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.buffer.resize(width * height, 0);
    }

    pub fn clear(&mut self, color: u32) {
        self.buffer.fill(color);
    }

    pub fn set_pixel(&mut self, x: i32, y: i32, color: u32) {
        if let Some((cx, cy, cw, ch)) = self.clip_rect {
            if x < cx || x >= cx + cw as i32 || y < cy || y >= cy + ch as i32 {
                return;
            }
        }
        if x >= 0 && (x as usize) < self.width && y >= 0 && (y as usize) < self.height {
            self.buffer[y as usize * self.width + x as usize] = color;
        }
    }

    pub fn draw_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: u32) {
        let (clip_min_x, clip_min_y, clip_max_x, clip_max_y) = if let Some((cx, cy, cw, ch)) = self.clip_rect {
            (
                cx.max(0) as usize,
                cy.max(0) as usize,
                ((cx + cw as i32).max(0) as usize).min(self.width),
                ((cy + ch as i32).max(0) as usize).min(self.height),
            )
        } else {
            (0, 0, self.width, self.height)
        };

        let x_start = (x.max(0) as usize).max(clip_min_x);
        let y_start = (y.max(0) as usize).max(clip_min_y);
        let x_end = (((x + w as i32).max(0) as usize).min(self.width)).min(clip_max_x);
        let y_end = (((y + h as i32).max(0) as usize).min(self.height)).min(clip_max_y);

        if x_start < x_end && y_start < y_end {
            for py in y_start..y_end {
                let row_offset = py * self.width;
                for px in x_start..x_end {
                    self.buffer[row_offset + px] = color;
                }
            }
        }
    }

    pub fn draw_rect_outline(&mut self, x: i32, y: i32, w: u32, h: u32, color: u32, thickness: u32) {
        let t = thickness as i32;
        // Top & Bottom
        self.draw_rect(x, y, w, thickness, color);
        self.draw_rect(x, y + h as i32 - t, w, thickness, color);
        // Left & Right
        self.draw_rect(x, y, thickness, h, color);
        self.draw_rect(x + w as i32 - t, y, thickness, h, color);
    }

    pub fn draw_circle(&mut self, cx: i32, cy: i32, radius: i32, color: u32) {
        if radius < 0 {
            return;
        }
        let r = radius;
        let r2 = (r as i64) * (r as i64);

        // Zone de dessin effective (clip rect ∩ bornes du canvas)
        let (eff_min_x, eff_min_y, eff_max_x, eff_max_y) = if let Some((ccx, ccy, ccw, cch)) =
            self.clip_rect
        {
            (
                ccx.max(0),
                ccy.max(0),
                ((ccx + ccw as i32).max(0)).min(self.width as i32),
                ((ccy + cch as i32).max(0)).min(self.height as i32),
            )
        } else {
            (0, 0, self.width as i32, self.height as i32)
        };

        // Remplissage par tranches horizontales (une écriture contiguë par ligne),
        // pixel-identique à l'ancienne boucle dx/dy (dx² + dy² <= r²).
        for dy in -r..=r {
            let y = cy + dy;
            if y < eff_min_y || y >= eff_max_y {
                continue;
            }
            let dy2 = (dy as i64) * (dy as i64);
            let mut half = ((r2 - dy2) as f64).sqrt().floor() as i32;
            // Correction d'arrondi exacte (2 itérations max en pratique)
            while (half as i64 + 1) * (half as i64 + 1) + dy2 <= r2 {
                half += 1;
            }
            while (half as i64) * (half as i64) + dy2 > r2 {
                half -= 1;
            }

            let x0 = (cx - half).max(eff_min_x);
            let x1 = (cx + half + 1).min(eff_max_x);
            if x0 < x1 {
                let row = y as usize * self.width;
                for x in x0..x1 {
                    self.buffer[row + x as usize] = color;
                }
            }
        }
    }

    /// Algorithme de tracé de ligne de Bresenham ultra-rapide et déterministe
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut curr_x = x0;
        let mut curr_y = y0;

        loop {
            self.set_pixel(curr_x, curr_y, color);
            if curr_x == x1 && curr_y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                curr_x += sx;
            }
            if e2 <= dx {
                err += dx;
                curr_y += sy;
            }
        }
    }

    /// Tracé de ligne épaisse pour les fils de connexion KVL
    pub fn draw_thick_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, thickness: i32, color: u32) {
        let offset = thickness / 2;
        for ox in -offset..=offset {
            for oy in -offset..=offset {
                self.draw_line(x0 + ox, y0 + oy, x1 + ox, y1 + oy, color);
            }
        }
    }

    /// Affiche une matrice de pixels (sprite 2D pixel-art) redimensionnée aux dimensions de destination (nearest-neighbor)
    /// avec support de la transparence (si pixel == 0, pixel ignoré).
    pub fn draw_pixel_matrix(
        &mut self,
        dest_x: i32,
        dest_y: i32,
        dest_w: u32,
        dest_h: u32,
        matrix: &[u32],
        matrix_w: u32,
        matrix_h: u32,
    ) {
        if matrix_w == 0 || matrix_h == 0 || dest_w == 0 || dest_h == 0 {
            return;
        }

        let (clip_min_x, clip_min_y, clip_max_x, clip_max_y) = if let Some((cx, cy, cw, ch)) = self.clip_rect {
            (
                cx.max(0) as usize,
                cy.max(0) as usize,
                ((cx + cw as i32).max(0) as usize).min(self.width),
                ((cy + ch as i32).max(0) as usize).min(self.height),
            )
        } else {
            (0, 0, self.width, self.height)
        };

        let x_start = (dest_x.max(0) as usize).max(clip_min_x);
        let y_start = (dest_y.max(0) as usize).max(clip_min_y);
        let x_end = (((dest_x + dest_w as i32).max(0) as usize).min(self.width)).min(clip_max_x);
        let y_end = (((dest_y + dest_h as i32).max(0) as usize).min(self.height)).min(clip_max_y);

        if x_start >= x_end || y_start >= y_end {
            return;
        }

        // Arithmétique à virgule fixe 16.16 : élimine toute division entière dans les boucles de pixels
        let step_x_fp = ((matrix_w as u64) << 16) / (dest_w as u64);
        let step_y_fp = ((matrix_h as u64) << 16) / (dest_h as u64);

        let initial_src_x_fp = (x_start as i32 - dest_x).max(0) as u64 * step_x_fp;
        let mut src_y_fp = (y_start as i32 - dest_y).max(0) as u64 * step_y_fp;
        let max_src_x = (matrix_w as usize).saturating_sub(1);
        let max_src_y = (matrix_h as usize).saturating_sub(1);

        for py in y_start..y_end {
            let row_offset = py * self.width;
            let src_y = ((src_y_fp >> 16) as usize).min(max_src_y);
            src_y_fp += step_y_fp;
            let src_row = src_y * matrix_w as usize;

            let mut src_x_fp = initial_src_x_fp;
            for px in x_start..x_end {
                let src_x = ((src_x_fp >> 16) as usize).min(max_src_x);
                src_x_fp += step_x_fp;
                let idx = src_row + src_x;
                if idx < matrix.len() {
                    let color = matrix[idx];
                    if color != 0 {
                        self.buffer[row_offset + px] = color;
                    }
                }
            }
        }
    }

    /// Affiche une image RGBA **prémultipliée** (`0xAARRGGBB`) redimensionnée
    /// (nearest-neighbor) avec alpha blending sur le fond existant.
    ///
    /// Le pixel `0` est transparent. Le chemin opaque (`alpha == 255`) écrit
    /// directement, sans multiplication. Aucune allocation dans le hot path.
    pub fn draw_pixel_matrix_alpha(
        &mut self,
        dest_x: i32,
        dest_y: i32,
        dest_w: u32,
        dest_h: u32,
        matrix: &[u32],
        matrix_w: u32,
        matrix_h: u32,
    ) {
        if matrix_w == 0 || matrix_h == 0 || dest_w == 0 || dest_h == 0 {
            return;
        }

        let (clip_min_x, clip_min_y, clip_max_x, clip_max_y) = if let Some((cx, cy, cw, ch)) = self.clip_rect {
            (
                cx.max(0) as usize,
                cy.max(0) as usize,
                ((cx + cw as i32).max(0) as usize).min(self.width),
                ((cy + ch as i32).max(0) as usize).min(self.height),
            )
        } else {
            (0, 0, self.width, self.height)
        };

        let x_start = (dest_x.max(0) as usize).max(clip_min_x);
        let y_start = (dest_y.max(0) as usize).max(clip_min_y);
        let x_end = (((dest_x + dest_w as i32).max(0) as usize).min(self.width)).min(clip_max_x);
        let y_end = (((dest_y + dest_h as i32).max(0) as usize).min(self.height)).min(clip_max_y);

        if x_start >= x_end || y_start >= y_end {
            return;
        }

        // Arithmétique à virgule fixe 16.16 : aucune division entière dans les boucles.
        let step_x_fp = ((matrix_w as u64) << 16) / (dest_w as u64);
        let step_y_fp = ((matrix_h as u64) << 16) / (dest_h as u64);

        let initial_src_x_fp = (x_start as i32 - dest_x).max(0) as u64 * step_x_fp;
        let mut src_y_fp = (y_start as i32 - dest_y).max(0) as u64 * step_y_fp;
        let max_src_x = (matrix_w as usize).saturating_sub(1);
        let max_src_y = (matrix_h as usize).saturating_sub(1);

        for py in y_start..y_end {
            let row_offset = py * self.width;
            let src_y = ((src_y_fp >> 16) as usize).min(max_src_y);
            src_y_fp += step_y_fp;
            let src_row = src_y * matrix_w as usize;

            let mut src_x_fp = initial_src_x_fp;
            for px in x_start..x_end {
                let src_x = ((src_x_fp >> 16) as usize).min(max_src_x);
                src_x_fp += step_x_fp;
                let idx = src_row + src_x;
                if idx < matrix.len() {
                    let c = matrix[idx];
                    if c != 0 {
                        let a = (c >> 24) & 0xFF;
                        if a != 0 {
                            let di = row_offset + px;
                            if a == 255 {
                                // Opaque : écriture directe (r,g,b déjà = couleur réelle).
                                self.buffer[di] = c & 0x00FF_FFFF;
                            } else {
                                // Blend prémultiplié : out = src + dst * (255 - a) / 255
                                let dst = self.buffer[di];
                                let inv = 255 - a;
                                let out_r = ((c >> 16) & 0xFF) + div255(((dst >> 16) & 0xFF) * inv);
                                let out_g = ((c >> 8) & 0xFF) + div255(((dst >> 8) & 0xFF) * inv);
                                let out_b = (c & 0xFF) + div255((dst & 0xFF) * inv);
                                self.buffer[di] = (out_r << 16) | (out_g << 8) | out_b;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Affiche une image RGBA prémultipliée (`0xAARRGGBB`) avec rotation + échelle
    /// autour d'un pivot, échantillonnage nearest-neighbor et alpha blending.
    ///
    /// `pivot_x/pivot_y` : pivot en coordonnées du bitmap source. `angle` en
    /// radians, `scale_x/scale_y` échelles, `dx/dy` position écran du pivot.
    /// Aucune allocation dans le hot path.
    pub fn draw_sprite_affine(
        &mut self,
        matrix: &[u32],
        mw: u32,
        mh: u32,
        pivot_x: f32,
        pivot_y: f32,
        angle: f32,
        scale_x: f32,
        scale_y: f32,
        dx: f32,
        dy: f32,
    ) {
        if mw == 0 || mh == 0 {
            return;
        }
        let cos = angle.cos();
        let sin = angle.sin();
        // Inverse : src = S^-1 * R^-1 * (screen - d) + pivot
        let m00 = cos / scale_x;
        let m01 = sin / scale_x;
        let m10 = -sin / scale_y;
        let m11 = cos / scale_y;

        // Boîte englobante écran du quadrilatère transformé
        let corners = [
            (0.0f32, 0.0f32),
            (mw as f32, 0.0),
            (0.0, mh as f32),
            (mw as f32, mh as f32),
        ];
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for (cx, cy) in corners {
            let rx = cx - pivot_x;
            let ry = cy - pivot_y;
            let sx = rx * scale_x;
            let sy = ry * scale_y;
            let tx = cos * sx - sin * sy + dx;
            let ty = sin * sx + cos * sy + dy;
            min_x = min_x.min(tx);
            min_y = min_y.min(ty);
            max_x = max_x.max(tx);
            max_y = max_y.max(ty);
        }

        let cx0 = (min_x.floor() as i32).max(0);
        let cx1 = (max_x.ceil() as i32).min(self.width as i32);
        let cy0 = (min_y.floor() as i32).max(0);
        let cy1 = (max_y.ceil() as i32).min(self.height as i32);

        let mw_i = mw as i32;
        let mh_i = mh as i32;
        let stride = mw as usize;

        for v in cy0..cy1 {
            for u in cx0..cx1 {
                let ex = u as f32 - dx;
                let ey = v as f32 - dy;
                let sxf = m00 * ex + m01 * ey + pivot_x;
                let syf = m10 * ex + m11 * ey + pivot_y;
                let si = sxf as i32;
                let sj = syf as i32;
                if si >= 0 && si < mw_i && sj >= 0 && sj < mh_i {
                    let c = matrix[sj as usize * stride + si as usize];
                    if c != 0 {
                        let a = (c >> 24) & 0xFF;
                        if a != 0 {
                            let di = (v as usize) * self.width + (u as usize);
                            if a == 255 {
                                self.buffer[di] = c & 0x00FF_FFFF;
                            } else {
                                let dst = self.buffer[di];
                                let inv = 255 - a;
                                let out_r = ((c >> 16) & 0xFF) + div255(((dst >> 16) & 0xFF) * inv);
                                let out_g = ((c >> 8) & 0xFF) + div255(((dst >> 8) & 0xFF) * inv);
                                let out_b = (c & 0xFF) + div255((dst & 0xFF) * inv);
                                self.buffer[di] = (out_r << 16) | (out_g << 8) | out_b;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Mélange de couleur alpha (alpha blending) pour l'anti-aliasing de police
    pub fn blend_pixel(&mut self, x: i32, y: i32, color: u32, alpha: u8) {
        if alpha == 0 {
            return;
        }
        if let Some((cx, cy, cw, ch)) = self.clip_rect {
            if x < cx || x >= cx + cw as i32 || y < cy || y >= cy + ch as i32 {
                return;
            }
        }
        if x >= 0 && (x as usize) < self.width && y >= 0 && (y as usize) < self.height {
            let idx = y as usize * self.width + x as usize;
            if alpha == 255 {
                self.buffer[idx] = color;
            } else {
                let bg = self.buffer[idx];
                let a = alpha as u32;
                let inv_a = 255 - a;

                let bg_r = (bg >> 16) & 0xFF;
                let bg_g = (bg >> 8) & 0xFF;
                let bg_b = bg & 0xFF;

                let fg_r = (color >> 16) & 0xFF;
                let fg_g = (color >> 8) & 0xFF;
                let fg_b = color & 0xFF;

                let out_r = div255(fg_r * a + bg_r * inv_a);
                let out_g = div255(fg_g * a + bg_g * inv_a);
                let out_b = div255(fg_b * a + bg_b * inv_a);

                self.buffer[idx] = (out_r << 16) | (out_g << 8) | out_b;
            }
        }
    }

    /// Mesure les dimensions d'un texte rendu avec une police vectorielle
    pub fn measure_text_styled(font: &fontdue::Font, text: &str, size: f32) -> (i32, i32) {
        let mut width = 0;
        let mut cur_w = 0;
        let mut lines = 1;
        for c in text.chars() {
            if c == '\n' {
                lines += 1;
                cur_w = 0;
                continue;
            }
            if c == '♥' || c == '♡' || font.lookup_glyph_index(c) == 0 {
                let scale = (size / 7.0).max(1.0) as usize;
                cur_w += (6 * scale) as i32;
            } else {
                let metrics = font.metrics(c, size);
                cur_w += metrics.advance_width.ceil() as i32;
            }
            width = width.max(cur_w);
        }
        let line_height = font
            .horizontal_line_metrics(size)
            .map(|m| (m.ascent - m.descent).ceil() as i32)
            .unwrap_or((size * 1.25) as i32);
        (width, lines * line_height)
    }

    /// Rendu de texte anti-aliasé haute fidélité (TrueType / OpenType via fontdue)
    pub fn draw_text_styled(&mut self, font: &fontdue::Font, start_x: i32, start_y: i32, text: &str, color: u32, size: f32) -> (i32, i32) {
        let mut cur_x = start_x;
        let mut cur_y = start_y;
        let line_metrics = font.horizontal_line_metrics(size);
        let ascent = line_metrics.map(|m| m.ascent.ceil() as i32).unwrap_or(size as i32);
        let line_height = line_metrics
            .map(|m| (m.ascent - m.descent + m.line_gap).ceil() as i32)
            .unwrap_or((size * 1.25) as i32);
        let mut max_w = 0;

        for c in text.chars() {
            if c == '\n' {
                cur_x = start_x;
                cur_y += line_height;
                continue;
            }

            // Fallback rétro si glyphe non supporté ou icônes spécifiques de cœur
            if c == '♥' || c == '♡' || font.lookup_glyph_index(c) == 0 {
                let scale = (size / 7.0).max(1.0) as usize;
                if let Some(glyph) = get_char_glyph(c) {
                    for (row_idx, row_bits) in glyph.iter().enumerate() {
                        for col_idx in 0..5 {
                            if (row_bits & (1 << (4 - col_idx))) != 0 {
                                for sy in 0..scale {
                                    for sx in 0..scale {
                                        self.set_pixel(
                                            cur_x + (col_idx * scale + sx) as i32,
                                            cur_y + (row_idx * scale + sy) as i32,
                                            color,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                cur_x += (6 * scale) as i32;
                max_w = max_w.max(cur_x - start_x);
                continue;
            }

            let mut advance_w = 0;
            with_cached_glyph(font, c, size, |metrics, bitmap| {
                let gx = cur_x + metrics.xmin;
                let baseline_y = cur_y + ascent;
                let gy = baseline_y - (metrics.height as i32 + metrics.ymin);

                for row in 0..metrics.height {
                    for col in 0..metrics.width {
                        let mut alpha = bitmap[row * metrics.width + col];
                        if alpha > 0 {
                            if alpha >= 180 {
                                alpha = 255;
                            }
                            self.blend_pixel(gx + col as i32, gy + row as i32, color, alpha);
                        }
                    }
                }
                advance_w = metrics.advance_width.ceil() as i32;
            });
            cur_x += advance_w;
            max_w = max_w.max(cur_x - start_x);
        }
        (max_w, (cur_y - start_y) + line_height)
    }

    /// Mesure les dimensions de texte selon l'échelle standard
    pub fn measure_text(&self, text: &str, scale: usize) -> (i32, i32) {
        let font = get_title_font();
        let pt_size = match scale {
            1 => 12.0,
            2 => 14.0,
            _ => 17.0,
        };
        Canvas::measure_text_styled(font, text, pt_size)
    }

    /// Rendu de texte stylisé avec Space Grotesk Bold par défaut
    pub fn draw_text(&mut self, start_x: i32, start_y: i32, text: &str, color: u32, scale: usize) {
        let font = get_title_font();
        let pt_size = match scale {
            1 => 12.0,
            2 => 14.0,
            _ => 17.0,
        };
        self.draw_text_styled(font, start_x, start_y, text, color, pt_size);
    }

    /// Rendu de texte monospacé avec JetBrains Mono (idéal pour console KVL & télémétrie)
    pub fn draw_text_mono(&mut self, start_x: i32, start_y: i32, text: &str, color: u32, size: f32) -> (i32, i32) {
        let font = get_mono_font();
        self.draw_text_styled(font, start_x, start_y, text, color, size)
    }

    /// Rendu de texte bitmap 5x7 historique (compatibilité tests)
    #[allow(dead_code)]
    pub fn draw_text_retro(&mut self, start_x: i32, start_y: i32, text: &str, color: u32, scale: usize) {
        let mut cur_x = start_x;
        for c in text.chars() {
            if c == '\n' {
                cur_x = start_x;
                continue;
            }
            if let Some(glyph) = get_char_glyph(c) {
                for (row_idx, row_bits) in glyph.iter().enumerate() {
                    for col_idx in 0..5 {
                        if (row_bits & (1 << (4 - col_idx))) != 0 {
                            for sy in 0..scale {
                                for sx in 0..scale {
                                    self.set_pixel(
                                        cur_x + (col_idx * scale + sx) as i32,
                                        start_y + (row_idx * scale + sy) as i32,
                                        color,
                                    );
                                }
                            }
                        }
                    }
                }
            }
            cur_x += (6 * scale) as i32;
        }
    }

    /// Dessine des coins technologiques biseautés / cornières pour encadrer des zones
    pub fn draw_corner_brackets(&mut self, x: i32, y: i32, w: u32, h: u32, len: i32, color: u32) {
        let x2 = x + w as i32 - 1;
        let y2 = y + h as i32 - 1;
        // Haut-Gauche
        self.draw_line(x, y, x + len, y, color);
        self.draw_line(x, y, x, y + len, color);
        // Haut-Droite
        self.draw_line(x2, y, x2 - len, y, color);
        self.draw_line(x2, y, x2, y + len, color);
        // Bas-Gauche
        self.draw_line(x, y2, x + len, y2, color);
        self.draw_line(x, y2, x, y2 - len, color);
        // Bas-Droite
        self.draw_line(x2, y2, x2 - len, y2, color);
        self.draw_line(x2, y2, x2, y2 - len, color);
    }

    /// Dessine un badge / pilule d'action avec Space Grotesk Bold centré pixel-perfect
    pub fn draw_badge(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        bg_col: u32,
        border_col: u32,
        text: &str,
        text_col: u32,
        scale: usize,
    ) {
        self.draw_rect(x, y, w, h, bg_col);
        self.draw_rect_outline(x, y, w, h, border_col, 1);
        let font = get_title_font();
        let pt_size = match scale {
            1 => 11.0,
            2 => 13.0,
            _ => 16.0,
        };
        let (text_w, text_h) = Canvas::measure_text_styled(font, text, pt_size);
        let tx = (x + (w as i32 - text_w) / 2).max(x + 4);
        let ty = y + (h as i32 - text_h) / 2 - 1;
        self.draw_text_styled(font, tx, ty, text, text_col, pt_size);
    }

    /// Rendu de texte de corps (Body) avec Space Grotesk Regular (lisibilité maximale)
    pub fn draw_text_body(&mut self, start_x: i32, start_y: i32, text: &str, color: u32, pt_size: f32) {
        let font = get_body_font();
        self.draw_text_styled(font, start_x, start_y, text, color, pt_size);
    }

    /// Dessine un badge / bouton d'action avec gestion interactive du survol
    pub fn draw_badge_interactive(
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        bg_col: u32,
        border_col: u32,
        text: &str,
        text_col: u32,
        scale: usize,
        is_hovered: bool,
    ) {
        let (actual_bg, actual_bd, actual_txt) = if is_hovered {
            (
                0x00243447, // Fond subtilement éclairci
                0x0038BDF8, // Bordure cyan éclatante
                0x00FFFFFF, // Texte blanc brillant
            )
        } else {
            (bg_col, border_col, text_col)
        };
        self.draw_rect(x, y, w, h, actual_bg);
        self.draw_rect_outline(x, y, w, h, actual_bd, if is_hovered { 2 } else { 1 });
        let font = get_title_font();
        let pt_size = match scale {
            1 => 11.0,
            2 => 13.0,
            _ => 16.0,
        };
        let (text_w, text_h) = Canvas::measure_text_styled(font, text, pt_size);
        let tx = (x + (w as i32 - text_w) / 2).max(x + 4);
        let ty = y + (h as i32 - text_h) / 2 - 1;
        self.draw_text_styled(font, tx, ty, text, actual_txt, pt_size);
    }

    /// Dessine une infobulle (Tooltip) élégante Dark Atelier au-dessus ou sous un point
    pub fn draw_tooltip(&mut self, target_x: i32, target_y: i32, title: &str, shortcut: &str, desc: &str) {
        let title_font = get_title_font();
        let body_font = get_body_font();

        let title_str = if shortcut.is_empty() {
            title.to_string()
        } else {
            format!("{} [{}]", title, shortcut)
        };

        let (tw, th) = Canvas::measure_text_styled(title_font, &title_str, 12.0);
        let (dw, dh) = if !desc.is_empty() {
            Canvas::measure_text_styled(body_font, desc, 11.0)
        } else {
            (0, 0)
        };

        let tooltip_w = (tw.max(dw) + 20) as u32;
        let tooltip_h = (th + if !desc.is_empty() { dh + 6 } else { 0 } + 12) as u32;

        let mut tx = target_x - (tooltip_w as i32 / 2);
        let mut ty = target_y - tooltip_h as i32 - 8;

        tx = tx.clamp(8, (self.width as i32) - tooltip_w as i32 - 8);
        if ty < 8 {
            ty = target_y + 36;
        }

        self.draw_rect(tx + 2, ty + 2, tooltip_w, tooltip_h, 0x00020617);
        self.draw_rect(tx, ty, tooltip_w, tooltip_h, 0x000F172A);
        self.draw_rect_outline(tx, ty, tooltip_w, tooltip_h, 0x0038BDF8, 1);
        self.draw_corner_brackets(tx, ty, tooltip_w, tooltip_h, 5, 0x0006B6D4);

        self.draw_text_styled(title_font, tx + 10, ty + 6, &title_str, 0x00F8FAFC, 12.0);

        if !desc.is_empty() {
            self.draw_text_styled(body_font, tx + 10, ty + 6 + th + 4, desc, 0x0094A3B8, 11.0);
        }
    }
}

pub static TITLE_FONT: std::sync::OnceLock<fontdue::Font> = std::sync::OnceLock::new();
pub static BODY_FONT: std::sync::OnceLock<fontdue::Font> = std::sync::OnceLock::new();
pub static MONO_FONT: std::sync::OnceLock<fontdue::Font> = std::sync::OnceLock::new();

pub fn get_title_font() -> &'static fontdue::Font {
    TITLE_FONT.get_or_init(|| {
        let bytes = include_bytes!("../../../assets/fonts/SpaceGrotesk-Bold.ttf");
        fontdue::Font::from_bytes(bytes as &[u8], fontdue::FontSettings::default())
            .expect("Chargement police Space Grotesk Bold")
    })
}

pub fn get_body_font() -> &'static fontdue::Font {
    BODY_FONT.get_or_init(|| {
        let bytes = include_bytes!("../../../assets/fonts/SpaceGrotesk.ttf");
        fontdue::Font::from_bytes(bytes as &[u8], fontdue::FontSettings::default())
            .expect("Chargement police Space Grotesk Regular")
    })
}

pub fn get_mono_font() -> &'static fontdue::Font {
    MONO_FONT.get_or_init(|| {
        let bytes = include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf");
        fontdue::Font::from_bytes(bytes as &[u8], fontdue::FontSettings::default())
            .expect("Chargement police JetBrains Mono Bold")
    })
}

/// Glyphes 5x7 stylisées pour chiffres, lettres, caractères accentués (UTF-8) et symboles majeurs
fn get_char_glyph(c: char) -> Option<[u8; 7]> {
    // 1. Prise en charge directe et stylisée des caractères accentués (français / européen)
    match c {
        // Accents aigus (É, é)
        'é' | 'É' => return Some([0b00010, 0b00100, 0b11111, 0b10000, 0b11110, 0b10000, 0b11111]),
        // Accents graves (È, è, À, à, Ù, ù)
        'è' | 'È' => return Some([0b01000, 0b00100, 0b11111, 0b10000, 0b11110, 0b10000, 0b11111]),
        'à' | 'À' => return Some([0b01000, 0b00100, 0b01110, 0b10001, 0b11111, 0b10001, 0b10001]),
        'ù' | 'Ù' => return Some([0b01000, 0b00100, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        // Accents circonflexes (Ê, ê, Â, â, Î, î, Ô, ô, Û, û)
        'ê' | 'Ê' => return Some([0b00100, 0b01010, 0b11111, 0b10000, 0b11110, 0b10000, 0b11111]),
        'â' | 'Â' => return Some([0b00100, 0b01010, 0b01110, 0b10001, 0b11111, 0b10001, 0b10001]),
        'î' | 'Î' => return Some([0b00100, 0b01010, 0b01110, 0b00100, 0b00100, 0b00100, 0b01110]),
        'ô' | 'Ô' => return Some([0b00100, 0b01010, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110]),
        'û' | 'Û' => return Some([0b00100, 0b01010, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        // Trémas (Ë, ë, Ä, ä, Ï, ï, Ö, ö, Ü, ü)
        'ë' | 'Ë' => return Some([0b01010, 0b00000, 0b11111, 0b10000, 0b11110, 0b10000, 0b11111]),
        'ä' | 'Ä' => return Some([0b01010, 0b00000, 0b01110, 0b10001, 0b11111, 0b10001, 0b10001]),
        'ï' | 'Ï' => return Some([0b01010, 0b00000, 0b01110, 0b00100, 0b00100, 0b00100, 0b01110]),
        'ö' | 'Ö' => return Some([0b01010, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110]),
        'ü' | 'Ü' => return Some([0b01010, 0b00000, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        // Cédille (Ç, ç)
        'ç' | 'Ç' => return Some([0b01110, 0b10001, 0b10000, 0b10000, 0b10001, 0b01110, 0b00110]),
        // Ligatures (œ, Œ, æ, Æ)
        'œ' | 'Œ' => return Some([0b01111, 0b10100, 0b10110, 0b10100, 0b10111, 0b10000, 0b01111]),
        'æ' | 'Æ' => return Some([0b01111, 0b10100, 0b11110, 0b10100, 0b10111, 0b10100, 0b10111]),
        // Symboles typographiques & monétaires
        '«' => return Some([0b00000, 0b01010, 0b10100, 0b01010, 0b00000, 0b00000, 0b00000]),
        '»' => return Some([0b00000, 0b01010, 0b00101, 0b01010, 0b00000, 0b00000, 0b00000]),
        '°' => return Some([0b01100, 0b10010, 0b01100, 0b00000, 0b00000, 0b00000, 0b00000]),
        '€' => return Some([0b01110, 0b10000, 0b11110, 0b10000, 0b11110, 0b10000, 0b01110]),
        _ => {}
    }

    let upper = c.to_ascii_uppercase();
    match upper {
        '0' => Some([0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110]),
        '1' => Some([0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
        '2' => Some([0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111]),
        '3' => Some([0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110]),
        '4' => Some([0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010]),
        '5' => Some([0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110]),
        '6' => Some([0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110]),
        '7' => Some([0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000]),
        '8' => Some([0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110]),
        '9' => Some([0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110]),
        'A' => Some([0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
        'B' => Some([0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110]),
        'C' => Some([0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110]),
        'D' => Some([0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100]),
        'E' => Some([0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111]),
        'F' => Some([0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000]),
        'G' => Some([0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110]),
        'H' => Some([0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
        'I' => Some([0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
        'J' => Some([0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100]),
        'K' => Some([0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001]),
        'L' => Some([0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111]),
        'M' => Some([0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001]),
        'N' => Some([0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001]),
        'O' => Some([0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        'P' => Some([0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000]),
        'Q' => Some([0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101]),
        'R' => Some([0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001]),
        'S' => Some([0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110]),
        'T' => Some([0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]),
        'U' => Some([0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        'V' => Some([0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100]),
        'W' => Some([0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001]),
        'X' => Some([0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001]),
        'Y' => Some([0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100]),
        'Z' => Some([0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111]),
        '>' => Some([0b10000, 0b01000, 0b00100, 0b00010, 0b00100, 0b01000, 0b10000]),
        '<' => Some([0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010]),
        '[' => Some([0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110]),
        ']' => Some([0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110]),
        '(' => Some([0b00100, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b00100]),
        ')' => Some([0b00100, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b00100]),
        ',' => Some([0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100, 0b01000]),
        '*' => Some([0b00000, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0b00000]),
        ':' => Some([0b00000, 0b00100, 0b00000, 0b00000, 0b00100, 0b00000, 0b00000]),
        '/' => Some([0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b00000, 0b00000]),
        '-' => Some([0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000]),
        '+' => Some([0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000]),
        '=' => Some([0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000]),
        '!' => Some([0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100, 0b00000]),
        '.' => Some([0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00000]),
        ' ' => Some([0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000]),
        '?' => Some([0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100]),
        '_' => Some([0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111]),
        '|' => Some([0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]),
        '#' => Some([0b01010, 0b11111, 0b01010, 0b01010, 0b11111, 0b01010, 0b00000]),
        '%' => Some([0b11001, 0b11010, 0b00100, 0b01000, 0b01011, 0b10011, 0b00000]),
        ';' => Some([0b00000, 0b00100, 0b00000, 0b00000, 0b00100, 0b00100, 0b01000]),
        '\'' => Some([0b00100, 0b00100, 0b01000, 0b00000, 0b00000, 0b00000, 0b00000]),
        '"' => Some([0b01010, 0b01010, 0b10100, 0b00000, 0b00000, 0b00000, 0b00000]),
        '{' => Some([0b00110, 0b00100, 0b00100, 0b01100, 0b00100, 0b00100, 0b00110]),
        '}' => Some([0b01100, 0b00100, 0b00100, 0b00110, 0b00100, 0b00100, 0b01100]),
        '$' => Some([0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100]),
        '~' => Some([0b00000, 0b01101, 0b10010, 0b00000, 0b00000, 0b00000, 0b00000]),
        '@' => Some([0b01110, 0b10001, 0b01101, 0b10101, 0b10110, 0b10000, 0b01111]),
        '♥' => Some([0b01010, 0b11111, 0b11111, 0b11111, 0b01110, 0b00100, 0b00000]),
        '♡' => Some([0b01010, 0b10101, 0b10001, 0b10001, 0b01010, 0b00100, 0b00000]),
        '▶' => Some([0b10000, 0b11000, 0b11100, 0b11110, 0b11100, 0b11000, 0b10000]),
        '◀' => Some([0b00001, 0b00011, 0b00111, 0b01111, 0b00111, 0b00011, 0b00001]),
        '▲' => Some([0b00100, 0b01110, 0b11111, 0b00000, 0b00000, 0b00000, 0b00000]),
        '▼' => Some([0b00000, 0b00000, 0b11111, 0b01110, 0b00100, 0b00000, 0b00000]),
        '✓' => Some([0b00000, 0b00001, 0b00010, 0b10100, 0b01000, 0b00000, 0b00000]),
        '•' | '●' => Some([0b00000, 0b01110, 0b11111, 0b11111, 0b11111, 0b01110, 0b00000]),
        '✕' => Some([0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b00000, 0b00000]),
        '★' => Some([0b00100, 0b11111, 0b01110, 0b10101, 0b00100, 0b00000, 0b00000]),
        '⚡' => Some([0b00110, 0b01100, 0b11111, 0b00110, 0b01100, 0b11000, 0b00000]),
        _ => Some([0b11111, 0b10001, 0b10101, 0b10101, 0b10001, 0b11111, 0b00000]), // Carré de fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accented_characters_glyphs_distinct_and_valid() {
        // Vérifie que les caractères accentués français possèdent des glyphes spécifiques et valides
        let accented = ['é', 'è', 'ê', 'ë', 'à', 'â', 'î', 'ï', 'ô', 'ù', 'û', 'ç', 'œ', '«', '»', '°', '€'];
        for c in accented {
            let glyph = get_char_glyph(c);
            assert!(glyph.is_some(), "Le caractère '{}' doit avoir un glyphe défini", c);
            let g = glyph.unwrap();
            // Le glyphe ne doit pas être le carré de fallback plein
            assert_ne!(
                g,
                [0b11111, 0b10001, 0b10101, 0b10101, 0b10001, 0b11111, 0b00000],
                "Le caractère '{}' ne doit pas être le carré de repli",
                c
            );
        }

        // Vérifie la distinction entre 'é' (aigu) et 'è' (grave)
        let glyph_e_acute = get_char_glyph('é').unwrap();
        let glyph_e_grave = get_char_glyph('è').unwrap();
        assert_ne!(glyph_e_acute, glyph_e_grave, "'é' et 'è' doivent avoir des glyphes distincts");

        // Vérifie la distinction entre 'E' (sans accent) et 'é' (avec accent)
        let glyph_e_standard = get_char_glyph('E').unwrap();
        assert_ne!(glyph_e_acute, glyph_e_standard, "'é' doit être distinct de 'E' standard");
    }

    #[test]
    fn test_draw_text_accented_string_no_crash() {
        let mut canvas = Canvas::new(200, 100);
        canvas.draw_text(10, 10, "HÉROS ÉPÉE CHÂTEAU FORÊT CŒUR «CLÉ» 25°C 50€", 0x00FFFFFF, 1);
        // Vérifie qu'au moins un pixel a été allumé
        let has_lit_pixels = canvas.buffer.iter().any(|&p| p == 0x00FFFFFF);
        assert!(has_lit_pixels, "Le texte accentué doit produire des pixels allumés");
    }

    #[test]
    fn test_glyph_cache_retrieval_and_warmup() {
        let font = get_title_font();
        // Vérifie que le warmup s'exécute sans erreur
        warmup_glyph_cache();

        // Récupère deux fois le même caractère et vérifie la cohérence
        let mut metrics_first = None;
        with_cached_glyph(font, 'A', 14.0, |m, _| {
            metrics_first = Some(*m);
        });

        let mut metrics_second = None;
        with_cached_glyph(font, 'A', 14.0, |m, _| {
            metrics_second = Some(*m);
        });

        assert_eq!(metrics_first, metrics_second);
        assert!(metrics_first.unwrap().width > 0);
    }

    #[test]
    fn test_draw_pixel_matrix_fixed_point_scaling() {
        let mut canvas = Canvas::new(32, 32);
        // Sprite 2x2 en damier
        let sprite = [
            0x00FF0000, 0x0000FF00,
            0x000000FF, 0x00FFFFFF,
        ];
        // Rendu mis à l'échelle 4x (8x8)
        canvas.draw_pixel_matrix(4, 4, 8, 8, &sprite, 2, 2);

        // Le pixel en haut à gauche du sprite doit être rouge
        assert_eq!(canvas.buffer[4 * 32 + 4], 0x00FF0000);
        // Le pixel en bas à droite du sprite (4+7, 4+7) doit être blanc
        assert_eq!(canvas.buffer[11 * 32 + 11], 0x00FFFFFF);
    }

    #[test]
    fn test_draw_pixel_matrix_alpha_opaque_and_semi() {
        let mut canvas = Canvas::new(2, 1);
        canvas.buffer.fill(0x00FFFFFF); // fond blanc
        // Opaque rouge prémultiplié (a=255) puis semi-transparent rouge (a=128)
        let sprite = [0xFFFF0000u32, 0x80800000u32];
        canvas.draw_pixel_matrix_alpha(0, 0, 2, 1, &sprite, 2, 1);
        assert_eq!(canvas.buffer[0], 0x00FF0000, "opaque: rouge pur");
        assert_eq!(canvas.buffer[1], 0x00FF7F7F, "semi: rouge sur blanc");
    }

    #[test]
    fn test_draw_pixel_matrix_alpha_transparent_noop() {
        let mut canvas = Canvas::new(1, 1);
        canvas.buffer[0] = 0x00123456;
        canvas.draw_pixel_matrix_alpha(0, 0, 1, 1, &[0u32], 1, 1);
        assert_eq!(canvas.buffer[0], 0x00123456, "pixel 0 ne doit pas écrire");
    }

    #[test]
    fn test_draw_pixel_matrix_alpha_out_of_bounds() {
        let mut canvas = Canvas::new(4, 4);
        // Sprite 2x2 opaque rouge dessiné partiellement hors écran (dest à (3,3))
        let sprite = [0xFFFF0000u32; 4];
        canvas.draw_pixel_matrix_alpha(3, 3, 4, 4, &sprite, 2, 2);
        // Seul le pixel (3,3) est visible et écrit
        assert_eq!(canvas.buffer[3 * 4 + 3], 0x00FF0000);
        // Le reste reste transparent
        assert_eq!(canvas.buffer[0], 0);
    }

    #[test]
    fn test_draw_sprite_affine_identity_matches_matrix_alpha() {
        let sprite = [0xFFFF0000u32, 0xFF00FF00, 0xFF0000FF, 0xFFFFFFFF]; // 2x2 opaque
        let mut a = Canvas::new(8, 8);
        let mut b = Canvas::new(8, 8);
        a.draw_pixel_matrix_alpha(2, 2, 4, 4, &sprite, 2, 2);
        // Affine identité : pivot (0,0), angle 0, échelle 2, pivot à (2,2)
        b.draw_sprite_affine(&sprite, 2, 2, 0.0, 0.0, 0.0, 2.0, 2.0, 2.0, 2.0);
        assert_eq!(a.buffer, b.buffer, "identité affine ≡ blit matriciel");
    }

    #[test]
    fn test_draw_sprite_affine_rotation_produces_pixels() {
        let sprite = [0xFFFF0000u32, 0xFF0000FFu32]; // 2x1 horizontal
        let mut c = Canvas::new(16, 16);
        c.draw_sprite_affine(
            &sprite, 2, 1, 1.0, 0.5,
            std::f32::consts::FRAC_PI_2, 1.0, 1.0, 8.0, 8.0,
        );
        let written = c.buffer.iter().filter(|&&p| p != 0).count();
        assert!(written > 0, "la rotation doit produire des pixels");
    }

    #[test]
    fn test_div255_exact_for_blend_range() {
        // Le numérateur du blending alpha est borné à [0, 255*255] = [0, 65025].
        for v in 0..=65025u32 {
            assert_eq!(div255(v), v / 255, "div255({v})");
        }
    }

    #[test]
    fn test_draw_circle_matches_bruteforce() {
        let cx = 20i32;
        let cy = 20i32;
        let radius = 9i32;
        let color = 0x00FF00FFu32;

        let mut reference = Canvas::new(40, 40);
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy <= radius * radius {
                    reference.set_pixel(cx + dx, cy + dy, color);
                }
            }
        }

        let mut canvas = Canvas::new(40, 40);
        canvas.draw_circle(cx, cy, radius, color);

        assert_eq!(canvas.buffer, reference.buffer);
    }
}
