use std::sync::{Arc, Mutex};
use tinyaudio::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundType {
    Coin,
    DoorOpen,
    DoorClose,
    AccessDenied,
    BonusFanfare,
    SpeedBoost,
    Sparkles,
    UiClick,
    WireCut,
    Alarm,
    RadarPing,
    Teleport,
    PlayerJump,
    LaserShoot,
    BlockPush,
    PlayerDamage,
    HealPickup,
    PlayerDeath,
    Typewriter,
    QuestSuccess,
    ItemPickup,
    PotionDrink,
    KeyUnlock,
    BossRoar,
    BossTelegraph,
    BossSlam,
}

struct ActiveVoice {
    sound: SoundType,
    cursor: usize,
    duration_samples: usize,
    volume: f32,
}

impl ActiveVoice {
    fn new(sound: SoundType, sample_rate: usize) -> Self {
        let duration_secs = match sound {
            SoundType::UiClick => 0.03,
            SoundType::WireCut => 0.08,
            SoundType::Coin => 0.22,
            SoundType::DoorOpen => 0.35,
            SoundType::DoorClose => 0.25,
            SoundType::AccessDenied => 0.30,
            SoundType::BonusFanfare => 0.55,
            SoundType::SpeedBoost => 0.40,
            SoundType::Sparkles => 0.35,
            SoundType::Alarm => 0.45,
            SoundType::RadarPing => 0.12,
            SoundType::Teleport => 0.35,
            SoundType::PlayerJump => 0.20,
            SoundType::LaserShoot => 0.15,
            SoundType::BlockPush => 0.16,
            SoundType::PlayerDamage => 0.22,
            SoundType::HealPickup => 0.32,
            SoundType::PlayerDeath => 0.60,
            SoundType::Typewriter => 0.025,
            SoundType::QuestSuccess => 0.45,
            SoundType::ItemPickup => 0.28,
            SoundType::PotionDrink => 0.35,
            SoundType::KeyUnlock => 0.25,
            SoundType::BossRoar => 0.65,
            SoundType::BossTelegraph => 0.30,
            SoundType::BossSlam => 0.40,
        };

        let duration_samples = (duration_secs * sample_rate as f32) as usize;
        Self {
            sound,
            cursor: 0,
            duration_samples,
            volume: 0.25,
        }
    }

    /// Synthèse procédurale d'un échantillon audio à l'instant t
    fn sample(&mut self, sample_rate: f32) -> f32 {
        if self.cursor >= self.duration_samples {
            return 0.0;
        }

        let t = self.cursor as f32 / sample_rate;
        let progress = self.cursor as f32 / self.duration_samples as f32;
        self.cursor += 1;

        let raw_wave = match self.sound {
            SoundType::Coin => {
                // Deux notes rétro : 987 Hz (B5) puis 1318 Hz (E6)
                let freq = if progress < 0.35 { 987.0 } else { 1318.0 };
                let decay = (1.0 - progress).powf(1.8);
                // Onde carrée douce
                let square = if (t * freq).fract() < 0.5 { 0.8 } else { -0.8 };
                let sine = (t * freq * std::f32::consts::TAU).sin();
                (square * 0.4 + sine * 0.6) * decay
            }

            SoundType::BonusFanfare => {
                // Arpège héroïque 4 notes : C5 (523 Hz), E5 (659 Hz), G5 (784 Hz), C6 (1046 Hz)
                let freq = if progress < 0.20 {
                    523.25
                } else if progress < 0.40 {
                    659.25
                } else if progress < 0.60 {
                    783.99
                } else {
                    1046.50
                };
                let decay = (1.0 - progress).powf(1.2);
                let sine = (t * freq * std::f32::consts::TAU).sin();
                let triangle = 2.0 * (2.0 * (t * freq - (t * freq + 0.5).floor()).abs()) - 1.0;
                (sine * 0.7 + triangle * 0.3) * decay
            }

            SoundType::DoorOpen => {
                // Glissando ascendant : 220 Hz -> 440 Hz
                let freq = 220.0 + progress * 220.0;
                let decay = if progress < 0.2 { progress / 0.2 } else { (1.0 - progress).powf(1.1) };
                let triangle = 2.0 * (2.0 * (t * freq - (t * freq + 0.5).floor()).abs()) - 1.0;
                triangle * decay * 0.8
            }

            SoundType::DoorClose => {
                // Glissando descendant : 380 Hz -> 140 Hz
                let freq = 380.0 - progress * 240.0;
                let decay = (1.0 - progress).powf(1.5);
                let sine = (t * freq * std::f32::consts::TAU).sin();
                sine * decay
            }

            SoundType::AccessDenied => {
                // Double bip d'erreur grave : 110 Hz / 140 Hz haché
                let phase_gate = ((progress * 10.0) as i32 % 2) == 0;
                if phase_gate {
                    let freq = 130.0;
                    let square = if (t * freq).fract() < 0.5 { 0.7 } else { -0.7 };
                    square * 0.6
                } else {
                    0.0
                }
            }

            SoundType::SpeedBoost => {
                // Balayage ascendant rapide avec vibrato
                let vibrato = (t * 30.0).sin() * 20.0;
                let freq = 300.0 + progress * 500.0 + vibrato;
                let decay = (1.0 - progress).powf(1.1);
                let saw = 2.0 * (t * freq).fract() - 1.0;
                saw * 0.4 * decay
            }

            SoundType::Sparkles => {
                // Sons étincelants aléatoires haute fréquence
                let freq = 1200.0 + ((self.cursor * 73) % 1200) as f32;
                let decay = (1.0 - progress).powf(2.0);
                let sine = (t * freq * std::f32::consts::TAU).sin();
                sine * decay * 0.7
            }

            SoundType::UiClick => {
                // Clic court haute fréquence
                let freq = 1200.0;
                let decay = (1.0 - progress).powf(4.0);
                let sine = (t * freq * std::f32::consts::TAU).sin();
                sine * decay
            }

            SoundType::WireCut => {
                // Bruit blanc / impulsion courte coupante
                let pseudo_noise = (((self.cursor * 1103515245 + 12345) % 100) as f32 / 50.0) - 1.0;
                let decay = (1.0 - progress).powf(3.0);
                pseudo_noise * decay * 0.6
            }

            SoundType::Alarm => {
                // Sirène d'alarme arcade 2 tons (600 Hz et 880 Hz)
                let phase = ((t * 8.0) as i32) % 2;
                let freq = if phase == 0 { 600.0 } else { 880.0 };
                let square = if (t * freq).fract() < 0.5 { 0.7 } else { -0.7 };
                let decay = 1.0 - progress * 0.3; // Maintien soutenu
                square * decay * 0.5
            }

            SoundType::RadarPing => {
                // Bip radar court futuriste (1500 Hz avec chute rapide)
                let freq = 1500.0;
                let decay = (1.0 - progress).powf(3.0);
                let sine = (t * freq * std::f32::consts::TAU).sin();
                sine * decay * 0.5
            }

            SoundType::Teleport => {
                // Balayage ascendant avec modulation FM rétro (téléportation sci-fi)
                let base_freq = 250.0 + progress * 800.0;
                let mod_freq = 30.0;
                let mod_idx = 100.0 * (1.0 - progress);
                let inst_freq = base_freq + mod_idx * (t * mod_freq * std::f32::consts::TAU).sin();
                let phase = t * inst_freq * std::f32::consts::TAU;
                let wave = phase.sin();
                let env = (1.0 - progress).powf(1.5);
                wave * env * 0.6
            }

            SoundType::PlayerJump => {
                // Sifflement rétro ascendant (160 Hz -> 440 Hz) pour le saut de plateforme
                let freq = 160.0 + (progress * 280.0);
                let square = if (t * freq).fract() < 0.5 { 0.7 } else { -0.7 };
                let sine = (t * freq * std::f32::consts::TAU).sin();
                let env = (1.0 - progress).powf(1.3);
                (square * 0.4 + sine * 0.6) * env * 0.7
            }

            SoundType::LaserShoot => {
                // Balayage laser descendant rapide (950 Hz -> 200 Hz)
                let freq = 950.0 - (progress * 750.0);
                let square = if (t * freq).fract() < 0.5 { 0.8 } else { -0.8 };
                let env = (1.0 - progress).powf(2.0);
                square * env * 0.6
            }

            SoundType::BlockPush => {
                // Frottement et raclement d'un bloc lourd (90 Hz avec bruit de contact)
                let freq = 90.0 - (progress * 35.0);
                let wave = (t * freq * std::f32::consts::TAU).sin();
                let noise = (((self.cursor * 214013 + 2531011) % 100) as f32 / 50.0) - 1.0;
                let env = (1.0 - progress).powf(1.8);
                (wave * 0.7 + noise * 0.3) * env * 0.6
            }

            SoundType::PlayerDamage => {
                // Impact sourd et rugueux avec bruit granuleux
                let freq = 280.0 - (progress * 200.0);
                let square = if (t * freq).fract() < 0.5 { 0.7 } else { -0.7 };
                let noise = (((self.cursor * 1664525 + 1013904223) % 100) as f32 / 50.0) - 1.0;
                let env = (1.0 - progress).powf(2.2);
                (square * 0.6 + noise * 0.4) * env * 0.7
            }

            SoundType::HealPickup => {
                // Arpège ascendant pur et étincelant (Do5 -> Mi5 -> Sol5 -> Do6)
                let step = (progress * 4.0) as usize;
                let freq = match step {
                    0 => 523.25,
                    1 => 659.25,
                    2 => 783.99,
                    _ => 1046.50,
                };
                let sine = (t * freq * std::f32::consts::TAU).sin();
                let env = (1.0 - (progress * 4.0).fract()).powf(1.4) * (1.0 - progress * 0.3);
                sine * env * 0.6
            }

            SoundType::PlayerDeath => {
                // Chute dramatique de gamme rétro (défaite)
                let step = (progress * 5.0) as usize;
                let freq = match step {
                    0 => 440.0,
                    1 => 370.0,
                    2 => 293.0,
                    3 => 220.0,
                    _ => 110.0,
                };
                let saw = 2.0 * (t * freq).fract() - 1.0;
                let env = (1.0 - progress).powf(1.1);
                saw * env * 0.65
            }

            SoundType::Typewriter => {
                // Micro-impulsion subtile pour l'effet machine à écrire rétro (750 Hz)
                let freq = 750.0 + ((self.cursor * 17) % 60) as f32;
                let decay = (1.0 - progress).powf(3.5);
                let sine = (t * freq * std::f32::consts::TAU).sin();
                sine * decay * 0.35
            }

            SoundType::QuestSuccess => {
                // Fanfare étincelante 3 notes majeures : Mi5 (659 Hz), La5 (880 Hz), Mi6 (1318 Hz)
                let step = (progress * 3.0) as usize;
                let freq = match step {
                    0 => 659.25,
                    1 => 880.00,
                    _ => 1318.51,
                };
                let sine = (t * freq * std::f32::consts::TAU).sin();
                let triangle = 2.0 * (2.0 * (t * freq - (t * freq + 0.5).floor()).abs()) - 1.0;
                let env = (1.0 - (progress * 3.0).fract()).powf(1.3) * (1.0 - progress * 0.2);
                (sine * 0.7 + triangle * 0.3) * env * 0.65
            }

            SoundType::ItemPickup => {
                // Carillon ascendant étincelant : Ré5 (587 Hz), La5 (880 Hz), Ré6 (1174 Hz), La6 (1760 Hz)
                let step = (progress * 4.0) as usize;
                let freq = match step {
                    0 => 587.33,
                    1 => 880.00,
                    2 => 1174.66,
                    _ => 1760.00,
                };
                let sine = (t * freq * std::f32::consts::TAU).sin();
                let bell_harmonic = (t * freq * 2.76 * std::f32::consts::TAU).sin() * 0.25;
                let env = (1.0 - (progress * 4.0).fract()).powf(1.6) * (1.0 - progress * 0.25);
                (sine * 0.75 + bell_harmonic) * env * 0.65
            }

            SoundType::PotionDrink => {
                // Glouglou rafraîchissant de potion (modulation sinusoïdale 360 Hz -> 580 Hz avec vibrato doux)
                let vibrato = (t * 26.0 * std::f32::consts::TAU).sin() * 90.0;
                let base_freq = 360.0 + (progress * 220.0) + vibrato;
                let sine = (t * base_freq * std::f32::consts::TAU).sin();
                let env = (1.0 - progress).powf(1.4) * (0.6 + 0.4 * (t * 18.0 * std::f32::consts::TAU).sin().abs());
                sine * env * 0.6
            }

            SoundType::KeyUnlock => {
                // Déclic mécanique double de serrure déverrouillée (480 Hz puis 780 Hz)
                let is_first = progress < 0.45;
                let freq = if is_first { 480.0 } else { 780.0 };
                let pulse_progress = if is_first { progress / 0.45 } else { (progress - 0.45) / 0.55 };
                let square = if (t * freq).fract() < 0.5 { 0.6 } else { -0.6 };
                let sine = (t * freq * std::f32::consts::TAU).sin();
                let env = (1.0 - pulse_progress).powf(2.2);
                (square * 0.4 + sine * 0.6) * env * 0.7
            }

            SoundType::BossRoar => {
                // Rugissement colossal guttural et grondant (onde complexe modulée 55 Hz -> 95 Hz avec distorsion)
                let mod_freq = 55.0 + (progress * 40.0) + (t * 14.0 * std::f32::consts::TAU).sin() * 15.0;
                let sub_sine = (t * (mod_freq * 0.5) * std::f32::consts::TAU).sin();
                let main_saw = 2.0 * ((t * mod_freq).fract()) - 1.0;
                let noise = (((t * 12345.67).sin() * 43758.5453).fract() - 0.5) * 0.35;
                let env = if progress < 0.15 {
                    progress / 0.15
                } else {
                    (1.0 - progress).powf(1.2)
                };
                ((sub_sine * 0.5 + main_saw * 0.3 + noise * 0.2) * env).clamp(-1.0, 1.0) * 0.85
            }

            SoundType::BossTelegraph => {
                // Impulsion de charge télégraphiée aiguë (sweep rapide ascendant 700 Hz -> 1400 Hz avec pulsation)
                let freq = 700.0 + progress * 700.0;
                let pulse = (t * 40.0 * std::f32::consts::TAU).sin().abs();
                let sine = (t * freq * std::f32::consts::TAU).sin();
                let env = (1.0 - (progress - 0.5).abs() * 1.5).clamp(0.0, 1.0);
                sine * pulse * env * 0.7
            }

            SoundType::BossSlam => {
                // Slam tellurique lourd : impact sourd (120 Hz chutant à 35 Hz) + grondement tellurique
                let freq = (120.0 - progress * 85.0).max(30.0);
                let sine = (t * freq * std::f32::consts::TAU).sin();
                let triangle = 2.0 * (2.0 * (t * freq - (t * freq + 0.5).floor()).abs()) - 1.0;
                let env = (1.0 - progress).powf(2.5);
                (sine * 0.7 + triangle * 0.3) * env * 0.9
            }
        };

        raw_wave * self.volume
    }

    fn is_finished(&self) -> bool {
        self.cursor >= self.duration_samples
    }
}

struct SharedAudioState {
    voices: Vec<ActiveVoice>,
    is_muted: bool,
    master_volume: f32,
    sample_rate: usize,
}

pub struct AudioEngine {
    state: Arc<Mutex<SharedAudioState>>,
    _output_device: Option<tinyaudio::OutputDevice>,
    pub is_available: bool,
}

impl AudioEngine {
    pub fn new() -> Self {
        let sample_rate = 44100;
        let state = Arc::new(Mutex::new(SharedAudioState {
            voices: Vec::new(),
            is_muted: false,
            master_volume: 0.5,
            sample_rate,
        }));

        let params = OutputDeviceParameters {
            channels_count: 1,
            sample_rate,
            channel_sample_count: 441, // ~10 ms buffer
        };

        let state_clone = Arc::clone(&state);
        let output_device = run_output_device(params, move |data: &mut [f32]| {
            if let Ok(mut st) = state_clone.lock() {
                if st.is_muted || st.voices.is_empty() {
                    for sample in data.iter_mut() {
                        *sample = 0.0;
                    }
                    return;
                }

                let s_rate = st.sample_rate as f32;
                let m_vol = st.master_volume;

                for sample in data.iter_mut() {
                    let mut mix = 0.0f32;
                    for voice in st.voices.iter_mut() {
                        mix += voice.sample(s_rate);
                    }
                    *sample = (mix * m_vol).clamp(-1.0, 1.0);
                }

                // Supprimer les voix terminées
                st.voices.retain(|v| !v.is_finished());
            } else {
                for sample in data.iter_mut() {
                    *sample = 0.0;
                }
            }
        });

        let is_available = output_device.is_ok();
        let _output_device = output_device.ok();

        Self {
            state,
            _output_device,
            is_available,
        }
    }

    pub fn play(&self, sound: SoundType) {
        if let Ok(mut st) = self.state.lock() {
            if st.voices.len() < 16 {
                let sample_rate = st.sample_rate;
                st.voices.push(ActiveVoice::new(sound, sample_rate));
            }
        }
    }

    pub fn toggle_mute(&self) -> bool {
        if let Ok(mut st) = self.state.lock() {
            st.is_muted = !st.is_muted;
            st.is_muted
        } else {
            false
        }
    }

    pub fn is_muted(&self) -> bool {
        if let Ok(st) = self.state.lock() {
            st.is_muted
        } else {
            false
        }
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_sound_types_synthesize_valid_samples() {
        let sounds = [
            SoundType::Coin,
            SoundType::DoorOpen,
            SoundType::DoorClose,
            SoundType::AccessDenied,
            SoundType::BonusFanfare,
            SoundType::SpeedBoost,
            SoundType::Sparkles,
            SoundType::UiClick,
            SoundType::WireCut,
            SoundType::Alarm,
            SoundType::RadarPing,
            SoundType::Teleport,
            SoundType::PlayerJump,
            SoundType::LaserShoot,
            SoundType::BlockPush,
            SoundType::PlayerDamage,
            SoundType::HealPickup,
            SoundType::PlayerDeath,
            SoundType::Typewriter,
            SoundType::QuestSuccess,
            SoundType::ItemPickup,
            SoundType::PotionDrink,
            SoundType::KeyUnlock,
            SoundType::BossRoar,
            SoundType::BossTelegraph,
            SoundType::BossSlam,
        ];

        let sample_rate = 44100.0;

        for sound in sounds {
            let mut voice = ActiveVoice::new(sound, sample_rate as usize);
            assert!(!voice.is_finished(), "Voice {:?} should not be finished initially", sound);

            let mut has_non_zero = false;
            let mut total_samples = 0;

            while !voice.is_finished() {
                let s = voice.sample(sample_rate);
                assert!(
                    s >= -1.0 && s <= 1.0,
                    "Sample for {:?} out of bounds: {}",
                    sound,
                    s
                );
                if s.abs() > 0.001 {
                    has_non_zero = true;
                }
                total_samples += 1;
            }

            assert!(
                has_non_zero,
                "Voice {:?} must generate non-zero audio samples",
                sound
            );
            assert!(
                total_samples > 100,
                "Voice {:?} must have a reasonable duration (>100 samples)",
                sound
            );
        }
    }

    #[test]
    fn test_audio_engine_play_and_mute() {
        let engine = AudioEngine::new();
        assert!(!engine.is_muted());

        let muted = engine.toggle_mute();
        assert!(muted);
        assert!(engine.is_muted());

        let unmuted = engine.toggle_mute();
        assert!(!unmuted);
        assert!(!engine.is_muted());

        // Queue sounds
        engine.play(SoundType::Coin);
        engine.play(SoundType::BonusFanfare);

        {
            let st = engine.state.lock().expect("lock state");
            assert_eq!(st.voices.len(), 2, "Two voices should be queued");
        }
    }
}
