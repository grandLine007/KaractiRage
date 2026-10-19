use std::collections::HashSet;
use winit::keyboard::KeyCode;

#[derive(Default)]
pub struct InputState {
    pub pressed_keys: HashSet<KeyCode>,
    pub just_pressed_keys: HashSet<KeyCode>,
    pub received_chars: Vec<char>,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub mouse_pressed: bool,
    pub mouse_just_clicked: bool,
    pub mouse_just_released: bool,
    pub mouse_right_just_clicked: bool,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_char(&mut self, ch: char) {
        self.received_chars.push(ch);
    }

    pub fn on_key_down(&mut self, key: KeyCode) {
        if !self.pressed_keys.contains(&key) {
            self.just_pressed_keys.insert(key);
        }
        self.pressed_keys.insert(key);
    }

    pub fn on_key_up(&mut self, key: KeyCode) {
        self.pressed_keys.remove(&key);
    }

    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        self.mouse_x = x;
        self.mouse_y = y;
    }

    pub fn on_mouse_down(&mut self) {
        if !self.mouse_pressed {
            self.mouse_just_clicked = true;
        }
        self.mouse_pressed = true;
    }

    pub fn on_mouse_up(&mut self) {
        if self.mouse_pressed {
            self.mouse_just_released = true;
        }
        self.mouse_pressed = false;
    }

    pub fn on_mouse_right_click(&mut self) {
        self.mouse_right_just_clicked = true;
    }

    pub fn clear_transient(&mut self) {
        self.just_pressed_keys.clear();
        self.received_chars.clear();
        self.mouse_just_clicked = false;
        self.mouse_just_released = false;
        self.mouse_right_just_clicked = false;
    }

    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.pressed_keys.contains(&key)
    }

    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.is_key_down(key)
    }

    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.just_pressed_keys.contains(&key)
    }
}
