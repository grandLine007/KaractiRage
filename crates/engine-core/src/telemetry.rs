use std::collections::VecDeque;

pub struct Telemetry {
    pub frame_times_us: VecDeque<u64>,
    pub max_samples: usize,
    pub current_fps: f32,
    pub avg_frame_time_us: u64,
    pub min_frame_time_us: u64,
    pub max_frame_time_us: u64,
    pub kvl_steps_last_frame: usize,
    pub show_overlay: bool,
}

impl Telemetry {
    pub fn new() -> Self {
        Self {
            frame_times_us: VecDeque::with_capacity(120),
            max_samples: 120,
            current_fps: 60.0,
            avg_frame_time_us: 0,
            min_frame_time_us: u64::MAX,
            max_frame_time_us: 0,
            kvl_steps_last_frame: 0,
            show_overlay: true, // Visible par défaut pour mesurer
        }
    }

    pub fn toggle_overlay(&mut self) {
        self.show_overlay = !self.show_overlay;
    }

    pub fn record_frame(&mut self, frame_duration_us: u64, kvl_steps: usize) {
        self.kvl_steps_last_frame = kvl_steps;

        if self.frame_times_us.len() >= self.max_samples {
            self.frame_times_us.pop_front();
        }
        self.frame_times_us.push_back(frame_duration_us);

        // Calculs statistiques
        if !self.frame_times_us.is_empty() {
            let sum: u64 = self.frame_times_us.iter().sum();
            self.avg_frame_time_us = sum / (self.frame_times_us.len() as u64);
            self.min_frame_time_us = *self.frame_times_us.iter().min().unwrap_or(&0);
            self.max_frame_time_us = *self.frame_times_us.iter().max().unwrap_or(&0);

            if self.avg_frame_time_us > 0 {
                self.current_fps = 1_000_000.0 / (self.avg_frame_time_us as f32);
            }
        }
    }
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::new()
    }
}
