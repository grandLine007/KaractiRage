#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityProfile {
    Compatibility,
    Balanced,
    High,
}

impl QualityProfile {
    pub fn name(&self) -> &'static str {
        match self {
            QualityProfile::Compatibility => "COMPATIBILITE (LOW-END)",
            QualityProfile::Balanced => "EQUILIBRE",
            QualityProfile::High => "ELEVE (RICHE)",
        }
    }

    pub fn color(&self) -> u32 {
        match self {
            QualityProfile::Compatibility => 0x0010B981, // Vert émeraude
            QualityProfile::Balanced => 0x003B82F6,      // Bleu
            QualityProfile::High => 0x00A855F7,          // Violet
        }
    }

    #[allow(dead_code)]
    pub fn render_scale(&self) -> f32 {
        match self {
            QualityProfile::Compatibility => 0.6, // Échelle allégée pour vieux CPU/GPU
            QualityProfile::Balanced => 0.85,
            QualityProfile::High => 1.0,
        }
    }

    pub fn max_particles(&self) -> usize {
        match self {
            QualityProfile::Compatibility => 10,
            QualityProfile::Balanced => 35,
            QualityProfile::High => 100,
        }
    }
}

pub struct AdaptationManager {
    pub current_profile: QualityProfile,
    pub auto_adapt: bool,
    pub target_fps: f32,
    pub target_budget_us: u64,
    pub overbudget_frames: usize,
    pub underbudget_frames: usize,
    pub cooldown_frames: usize,
}

impl AdaptationManager {
    pub fn new() -> Self {
        Self {
            current_profile: QualityProfile::Compatibility,
            auto_adapt: true,
            target_fps: 60.0,
            target_budget_us: 16_666, // 16.6ms pour 60 FPS
            overbudget_frames: 0,
            underbudget_frames: 0,
            cooldown_frames: 0,
        }
    }

    pub fn cycle_profile_manual(&mut self) {
        self.auto_adapt = false;
        self.current_profile = match self.current_profile {
            QualityProfile::Compatibility => QualityProfile::Balanced,
            QualityProfile::Balanced => QualityProfile::High,
            QualityProfile::High => QualityProfile::Compatibility,
        };
    }

    pub fn toggle_auto_mode(&mut self) {
        self.auto_adapt = !self.auto_adapt;
        self.cooldown_frames = 60; // Hystérésis
    }

    /// Boucle d'adaptation déterministe avec hystérésis (Section 5.4 de la vision technique)
    pub fn evaluate_adaptation(&mut self, current_frame_time_us: u64) -> Option<&'static str> {
        if !self.auto_adapt {
            return None;
        }

        if self.cooldown_frames > 0 {
            self.cooldown_frames -= 1;
            return None;
        }

        let threshold_high_us = self.target_budget_us + 2_000; // > 18.6ms
        let threshold_low_us = self.target_budget_us.saturating_sub(6_000); // < 10.6ms

        if current_frame_time_us > threshold_high_us {
            self.overbudget_frames += 1;
            self.underbudget_frames = 0;

            // Si surcharge constatée sur 30 frames consécutives -> Dégradation gracieuse
            if self.overbudget_frames >= 30 {
                self.overbudget_frames = 0;
                self.cooldown_frames = 90; // Pause de 1.5s pour stabiliser

                match self.current_profile {
                    QualityProfile::High => {
                        self.current_profile = QualityProfile::Balanced;
                        return Some("Adaptation : Passage automatique au profil EQUILIBRE (dépassement budget).");
                    }
                    QualityProfile::Balanced => {
                        self.current_profile = QualityProfile::Compatibility;
                        return Some("Adaptation : Dégradation automatique au profil COMPATIBILITE (maintien fluidité).");
                    }
                    QualityProfile::Compatibility => {
                        // Déjà au niveau le plus économique
                        return None;
                    }
                }
            }
        } else if current_frame_time_us < threshold_low_us {
            self.underbudget_frames += 1;
            self.overbudget_frames = 0;

            // Si machine très à l'aise pendant 120 frames -> Montée en gamme progressive
            if self.underbudget_frames >= 120 {
                self.underbudget_frames = 0;
                self.cooldown_frames = 120;

                match self.current_profile {
                    QualityProfile::Compatibility => {
                        self.current_profile = QualityProfile::Balanced;
                        return Some("Adaptation : Fluidité excellente, montée automatique vers EQUILIBRE.");
                    }
                    QualityProfile::Balanced => {
                        self.current_profile = QualityProfile::High;
                        return Some("Adaptation : Ressources abondantes, montée automatique vers ELEVE.");
                    }
                    QualityProfile::High => {
                        return None;
                    }
                }
            }
        } else {
            self.overbudget_frames = 0;
            self.underbudget_frames = 0;
        }

        None
    }
}
