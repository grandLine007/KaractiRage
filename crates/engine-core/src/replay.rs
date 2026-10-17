use serde::{Deserialize, Serialize};

/// Masque binaire compact des entrées du joueur pour la simulation déterministe
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FrameInputSnapshot {
    pub keys_bitmask: u32,
    pub mouse_x: i16,
    pub mouse_y: i16,
    pub mouse_clicked: bool,
}

impl FrameInputSnapshot {
    pub const KEY_LEFT: u32 = 1 << 0;
    pub const KEY_RIGHT: u32 = 1 << 1;
    pub const KEY_UP: u32 = 1 << 2;
    pub const KEY_DOWN: u32 = 1 << 3;
    pub const KEY_JUMP: u32 = 1 << 4;
    pub const KEY_FIRE: u32 = 1 << 5;
    pub const KEY_INTERACT: u32 = 1 << 6;
    #[allow(dead_code)]
    pub const KEY_RUN: u32 = 1 << 7;

    #[inline]
    #[allow(dead_code)]
    pub fn is_pressed(&self, mask: u32) -> bool {
        (self.keys_bitmask & mask) != 0
    }

    #[inline]
    pub fn set_key(&mut self, mask: u32, pressed: bool) {
        if pressed {
            self.keys_bitmask |= mask;
        } else {
            self.keys_bitmask &= !mask;
        }
    }
}

/// Session de Replay Déterministe sérialisable (Golden Replay & Ghost Run)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReplaySession {
    pub scene_path: String,
    pub initial_state_hash: u64,
    pub frames: Vec<FrameInputSnapshot>,
    pub ghost_samples: Vec<(f32, f32)>,
    pub final_state_hash: u64,
    pub total_score: i64,
    pub duration_seconds: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayMode {
    Idle,
    Recording,
    Playing,
}

/// Gestionnaire de Replay Déterministe et Joueur Fantôme (Ghost Player)
#[derive(Debug, Clone)]
pub struct ReplayManager {
    pub mode: ReplayMode,
    pub active_session: Option<ReplaySession>,
    pub current_frame_index: usize,
    pub ghost_sample_interval: usize,
    pub frame_counter: usize,
    pub show_ghost: bool,
    pub status_notification: Option<String>,
}

impl Default for ReplayManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayManager {
    pub fn new() -> Self {
        Self {
            mode: ReplayMode::Idle,
            active_session: None,
            current_frame_index: 0,
            ghost_sample_interval: 4, // 1 échantillon fantôme toutes les 4 frames (~15 Hz)
            frame_counter: 0,
            show_ghost: true,
            status_notification: None,
        }
    }

    /// Démarre l'enregistrement d'une session de jeu
    pub fn start_recording(&mut self, scene_path: &str, initial_hash: u64) {
        self.mode = ReplayMode::Recording;
        self.current_frame_index = 0;
        self.frame_counter = 0;
        self.active_session = Some(ReplaySession {
            scene_path: scene_path.to_string(),
            initial_state_hash: initial_hash,
            frames: Vec::with_capacity(3600), // Pré-réservation pour 60s à 60 FPS
            ghost_samples: Vec::with_capacity(900),
            final_state_hash: 0,
            total_score: 0,
            duration_seconds: 0.0,
        });
        self.status_notification = Some("Replay : Enregistrement démarré [REC]".into());
    }

    /// Enregistre un tick de simulation dans la session active
    pub fn record_tick(&mut self, snapshot: FrameInputSnapshot, player_pos: (f32, f32)) {
        if self.mode != ReplayMode::Recording {
            return;
        }

        if let Some(ref mut session) = self.active_session {
            session.frames.push(snapshot);
            if self.frame_counter % self.ghost_sample_interval == 0 {
                session.ghost_samples.push(player_pos);
            }
            self.frame_counter += 1;
            session.duration_seconds = self.frame_counter as f32 / 60.0;
        }
    }

    /// Arrête l'enregistrement et fige le hash de fin
    pub fn stop_recording(&mut self, final_hash: u64, score: i64) -> Option<&ReplaySession> {
        if self.mode != ReplayMode::Recording {
            return None;
        }

        self.mode = ReplayMode::Idle;
        if let Some(ref mut session) = self.active_session {
            session.final_state_hash = final_hash;
            session.total_score = score;
            let frames_count = session.frames.len();
            self.status_notification = Some(format!(
                "Replay sauvegardé : {} frames ({:.1}s), Score: {}",
                frames_count, session.duration_seconds, score
            ));
        }

        self.active_session.as_ref()
    }

    /// Démarre la relecture de la session en mémoire
    #[allow(dead_code)]
    pub fn start_playback(&mut self) -> bool {
        if let Some(ref session) = self.active_session {
            if session.frames.is_empty() {
                return false;
            }
            self.mode = ReplayMode::Playing;
            self.current_frame_index = 0;
            self.status_notification = Some("Replay : Lecture en cours [PLAY]".into());
            true
        } else {
            false
        }
    }

    /// Arrête la relecture en cours
    pub fn stop_playback(&mut self) {
        if self.mode == ReplayMode::Playing {
            self.mode = ReplayMode::Idle;
            self.current_frame_index = 0;
            self.status_notification = Some("Replay : Lecture terminée".into());
        }
    }

    /// Récupère l'entrée joueur pour la frame courante pendant la relecture
    #[allow(dead_code)]
    pub fn next_playback_input(&mut self) -> Option<FrameInputSnapshot> {
        if self.mode != ReplayMode::Playing {
            return None;
        }

        if let Some(ref session) = self.active_session {
            if self.current_frame_index < session.frames.len() {
                let input = session.frames[self.current_frame_index];
                self.current_frame_index += 1;
                Some(input)
            } else {
                self.stop_playback();
                None
            }
        } else {
            None
        }
    }

    /// Récupère la position du fantôme correspondant au tick actuel
    pub fn current_ghost_position(&self, tick: usize) -> Option<(f32, f32)> {
        if !self.show_ghost {
            return None;
        }

        let session = self.active_session.as_ref()?;
        let sample_idx = tick / self.ghost_sample_interval;
        if sample_idx < session.ghost_samples.len() {
            Some(session.ghost_samples[sample_idx])
        } else {
            session.ghost_samples.last().copied()
        }
    }

    /// Alterne l'affichage du joueur fantôme
    pub fn toggle_ghost(&mut self) -> bool {
        self.show_ghost = !self.show_ghost;
        self.show_ghost
    }
}

/// Algorithme FNV-1a 64-bit déterministe pour le calcul de hash d'état de simulation
#[inline]
pub fn compute_deterministic_state_hash(
    tick: u64,
    player_x: f32,
    player_y: f32,
    player_hp: f32,
    score: i64,
    boss_hp: Option<f32>,
) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;

    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            hash ^= b as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    };

    feed(&tick.to_le_bytes());
    feed(&player_x.to_bits().to_le_bytes());
    feed(&player_y.to_bits().to_le_bytes());
    feed(&player_hp.to_bits().to_le_bytes());
    feed(&score.to_le_bytes());

    if let Some(bhp) = boss_hp {
        feed(&bhp.to_bits().to_le_bytes());
    }

    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_record_and_playback_lifecycle() {
        let mut manager = ReplayManager::new();
        let initial_hash = compute_deterministic_state_hash(0, 100.0, 200.0, 10.0, 0, None);

        manager.start_recording("assets/scenes/level_01.scene.json", initial_hash);
        assert_eq!(manager.mode, ReplayMode::Recording);

        // Enregistrer 30 frames
        for i in 0..30 {
            let mut snapshot = FrameInputSnapshot::default();
            if i % 2 == 0 {
                snapshot.set_key(FrameInputSnapshot::KEY_RIGHT, true);
            }
            if i > 15 {
                snapshot.set_key(FrameInputSnapshot::KEY_FIRE, true);
            }
            manager.record_tick(snapshot, (100.0 + i as f32 * 2.0, 200.0));
        }

        let final_hash = compute_deterministic_state_hash(30, 160.0, 200.0, 10.0, 50, None);
        let session = manager.stop_recording(final_hash, 50).expect("Session générée");
        assert_eq!(session.frames.len(), 30);
        assert_eq!(session.final_state_hash, final_hash);
        assert_eq!(manager.mode, ReplayMode::Idle);

        // Démarrer la lecture
        assert!(manager.start_playback());
        assert_eq!(manager.mode, ReplayMode::Playing);

        for i in 0..30 {
            let input = manager.next_playback_input().expect("Entrée présente");
            if i % 2 == 0 {
                assert!(input.is_pressed(FrameInputSnapshot::KEY_RIGHT));
            } else {
                assert!(!input.is_pressed(FrameInputSnapshot::KEY_RIGHT));
            }
            if i > 15 {
                assert!(input.is_pressed(FrameInputSnapshot::KEY_FIRE));
            }
        }

        // Fin de session
        assert_eq!(manager.next_playback_input(), None);
        assert_eq!(manager.mode, ReplayMode::Idle);
    }

    #[test]
    fn test_deterministic_state_hash_repeatability() {
        let h1 = compute_deterministic_state_hash(120, 340.5, 512.25, 8.0, 150, Some(45.0));
        let h2 = compute_deterministic_state_hash(120, 340.5, 512.25, 8.0, 150, Some(45.0));
        let h3 = compute_deterministic_state_hash(121, 340.5, 512.25, 8.0, 150, Some(45.0));

        assert_eq!(h1, h2, "Le hash d'état doit être 100% déterministe");
        assert_ne!(h1, h3, "Un tick différent doit produire un hash différent");
    }
}
