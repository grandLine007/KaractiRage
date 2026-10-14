pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub buffer: Vec<u32>,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            buffer: vec![0; width * height],
        }
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
        if x >= 0 && (x as usize) < self.width && y >= 0 && (y as usize) < self.height {
            self.buffer[y as usize * self.width + x as usize] = color;
        }
    }

    pub fn draw_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: u32) {
        let x_start = x.max(0) as usize;
        let y_start = y.max(0) as usize;
        let x_end = ((x + w as i32).max(0) as usize).min(self.width);
        let y_end = ((y + h as i32).max(0) as usize).min(self.height);

        for py in y_start..y_end {
            let row_offset = py * self.width;
            for px in x_start..x_end {
                self.buffer[row_offset + px] = color;
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
        let r2 = radius * radius;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy <= r2 {
                    self.set_pixel(cx + dx, cy + dy, color);
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

    /// Rendu de texte bitmap 5x7 intégré (autonome, sans dépendance TTF externe)
    pub fn draw_text(&mut self, start_x: i32, start_y: i32, text: &str, color: u32, scale: usize) {
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
}

/// Glyphes 5x7 basiques pour chiffres, lettres et symboles majeurs
fn get_char_glyph(c: char) -> Option<[u8; 7]> {
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
        _ => Some([0b11111, 0b10001, 0b10101, 0b10101, 0b10001, 0b11111, 0b00000]), // Carré de fallback
    }
}
