use std::collections::HashSet;
use winit::keyboard::KeyCode;

#[derive(Default)]
#[allow(dead_code)]
pub struct InputState {
    pub pressed_keys: HashSet<KeyCode>,
    pub just_pressed_keys: HashSet<KeyCode>,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub mouse_pressed: bool,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
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

    pub fn clear_transient(&mut self) {
        self.just_pressed_keys.clear();
    }

    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.pressed_keys.contains(&key)
    }

    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.just_pressed_keys.contains(&key)
    }
}
