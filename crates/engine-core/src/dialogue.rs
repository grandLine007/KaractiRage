use crate::audio::{AudioEngine, SoundType};
use crate::input::InputState;
use crate::renderer::Canvas;
use serde::{Deserialize, Serialize};

/// Événement renvoyé lors de l'interaction utilisateur avec la boîte de dialogue
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogueAction {
    None,
    Advanced,
    ChoiceSelected(usize), // 1 ou 2
    Closed,
}

/// Moteur de Boîte de Dialogue, Effet Machine à Écrire et Choix Multiples
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DialogueBox {
    pub is_active: bool,
    pub speaker: String,
    pub full_text: String,
    pub revealed_chars: usize,
    pub type_timer: f32,
    pub chars_per_second: f32,
    pub choices: Vec<String>,
    pub selected_choice: Option<usize>,
    pub avatar_label: String,
    pub avatar_color: u32,
    pub sound_timer: f32,
    #[serde(skip)]
    pub choice_1_hitbox: Option<(i32, i32, u32, u32)>,
    #[serde(skip)]
    pub choice_2_hitbox: Option<(i32, i32, u32, u32)>,
    #[serde(skip)]
    pub advance_btn_hitbox: Option<(i32, i32, u32, u32)>,
}

impl Default for DialogueBox {
    fn default() -> Self {
        Self::new()
    }
}

impl DialogueBox {
    pub fn new() -> Self {
        Self {
            is_active: false,
            speaker: String::new(),
            full_text: String::new(),
            revealed_chars: 0,
            type_timer: 0.0,
            chars_per_second: 38.0, // 38 caractères/seconde pour une lecture rythmée
            choices: Vec::new(),
            selected_choice: None,
            avatar_label: "NPC".to_string(),
            avatar_color: 0x0038BDF8,
            sound_timer: 0.0,
            choice_1_hitbox: None,
            choice_2_hitbox: None,
            advance_btn_hitbox: None,
        }
    }

    /// Ouvre un dialogue avec texte, avatar et options interactives optionnelles
    pub fn open(
        &mut self,
        speaker: impl Into<String>,
        text: impl Into<String>,
        choices: Vec<String>,
        avatar_label: impl Into<String>,
        avatar_color: u32,
    ) {
        self.is_active = true;
        self.speaker = speaker.into();
        self.full_text = text.into();
        self.revealed_chars = 0;
        self.type_timer = 0.0;
        self.choices = choices;
        self.selected_choice = None;
        self.avatar_label = avatar_label.into();
        self.avatar_color = avatar_color;
        self.sound_timer = 0.0;
        self.choice_1_hitbox = None;
        self.choice_2_hitbox = None;
        self.advance_btn_hitbox = None;
    }

    /// Ferme la boîte de dialogue en cours
    pub fn close(&mut self) {
        self.is_active = false;
        self.choice_1_hitbox = None;
        self.choice_2_hitbox = None;
        self.advance_btn_hitbox = None;
    }

    /// Indique si le texte est encore en cours de saisie par la machine à écrire
    #[inline]
    pub fn is_typing(&self) -> bool {
        self.revealed_chars < self.full_text.chars().count()
    }

    /// Accélère instantanément et affiche tout le texte sans délai
    pub fn fast_forward(&mut self) {
        self.revealed_chars = self.full_text.chars().count();
    }

    /// Met à jour l'effet machine à écrire et synthétise les sons de frappe
    pub fn update(&mut self, dt: f32, audio: &mut AudioEngine) {
        if !self.is_active {
            return;
        }

        self.sound_timer = (self.sound_timer - dt).max(0.0);

        if self.is_typing() {
            self.type_timer += dt;
            let step = 1.0 / self.chars_per_second.max(1.0);

            while self.type_timer >= step && self.is_typing() {
                self.type_timer -= step;
                self.revealed_chars += 1;

                // Jouer le son de machine à écrire avec limitation de cadence (~45ms)
                if self.sound_timer <= 0.0 {
                    audio.play(SoundType::Typewriter);
                    self.sound_timer = 0.045;
                }
            }
        }
    }

    /// Traite les touches clavier et clics souris pour la progression du dialogue
    pub fn handle_input(&mut self, input: &InputState) -> DialogueAction {
        if !self.is_active {
            return DialogueAction::None;
        }

        let is_typing = self.is_typing();

        let mx = input.mouse_x as i32;
        let my = input.mouse_y as i32;
        let mouse_clicked = input.mouse_just_clicked;

        // 1. Si le texte défile encore, toute action valide accélère et révèle tout
        let any_confirm = input.is_key_just_pressed(winit::keyboard::KeyCode::Space)
            || input.is_key_just_pressed(winit::keyboard::KeyCode::Enter)
            || input.is_key_just_pressed(winit::keyboard::KeyCode::KeyE);

        if is_typing {
            if any_confirm || mouse_clicked {
                self.fast_forward();
                return DialogueAction::Advanced;
            }
            return DialogueAction::None;
        }

        // 2. Texte entièrement révélé : gestion des choix
        if !self.choices.is_empty() {
            // Raccourci Clavier [1] ou clic sur Choix 1
            let clicked_c1 = if let Some((bx, by, bw, bh)) = self.choice_1_hitbox {
                mouse_clicked && mx >= bx && mx < bx + bw as i32 && my >= by && my < by + bh as i32
            } else {
                false
            };
            if input.is_key_just_pressed(winit::keyboard::KeyCode::Digit1)
                || input.is_key_just_pressed(winit::keyboard::KeyCode::Numpad1)
                || clicked_c1
            {
                self.selected_choice = Some(1);
                self.close();
                return DialogueAction::ChoiceSelected(1);
            }

            // Raccourci Clavier [2] ou clic sur Choix 2
            let clicked_c2 = if let Some((bx, by, bw, bh)) = self.choice_2_hitbox {
                mouse_clicked && mx >= bx && mx < bx + bw as i32 && my >= by && my < by + bh as i32
            } else {
                false
            };
            if self.choices.len() >= 2
                && (input.is_key_just_pressed(winit::keyboard::KeyCode::Digit2)
                    || input.is_key_just_pressed(winit::keyboard::KeyCode::Numpad2)
                    || clicked_c2)
            {
                self.selected_choice = Some(2);
                self.close();
                return DialogueAction::ChoiceSelected(2);
            }
        } else {
            // Dialogue simple sans choix : fermeture / continuation
            let clicked_adv = if let Some((bx, by, bw, bh)) = self.advance_btn_hitbox {
                mouse_clicked && mx >= bx && mx < bx + bw as i32 && my >= by && my < by + bh as i32
            } else {
                false
            };
            if any_confirm || clicked_adv {
                self.close();
                return DialogueAction::Closed;
            }
        }

        DialogueAction::None
    }

    /// Rendu de la boîte de dialogue stylisée rétro / sci-fi sans allocation sur le heap
    pub fn render(&mut self, canvas: &mut Canvas) {
        if !self.is_active {
            return;
        }

        let w = canvas.width as i32;
        let h = canvas.height as i32;

        // Dimensions et position de la carte de dialogue
        let card_w = (w - 120).clamp(420, 920) as u32;
        let card_h = 175u32;
        let card_x = ((w - card_w as i32) / 2).max(10);
        let card_y = (h - card_h as i32 - 28).max(65);

        // 1. Fond sombre glassmorphism et double bordure lumineuse
        canvas.draw_rect(card_x, card_y, card_w, card_h, 0x000F172A);
        canvas.draw_rect_outline(card_x, card_y, card_w, card_h, 0x001E293B, 2);
        canvas.draw_rect_outline(card_x + 2, card_y + 2, card_w - 4, card_h - 4, 0x000284C7, 1);
        canvas.draw_corner_brackets(card_x, card_y, card_w, card_h, 18, 0x0038BDF8);

        // 2. Avatar / Portrait Badge
        let av_x = card_x + 16;
        let av_y = card_y + 16;
        let av_w = 64u32;
        let av_h = 64u32;
        canvas.draw_rect(av_x, av_y, av_w, av_h, 0x0018181B);
        canvas.draw_rect_outline(av_x, av_y, av_w, av_h, self.avatar_color, 2);
        canvas.draw_circle(av_x + 32, av_y + 32, 22, 0x000F172A);
        canvas.draw_circle(av_x + 32, av_y + 32, 16, self.avatar_color);

        // Label court de l'avatar (ex: PNJ, G, SAGE)
        let av_lbl = if self.avatar_label.is_empty() { "NPC" } else { &self.avatar_label };
        let av_lbl_w = (av_lbl.len() * 12) as i32;
        let av_lbl_x = (av_x + (64 - av_lbl_w) / 2).max(av_x + 4);
        canvas.draw_text(av_lbl_x, av_y + 72, av_lbl, self.avatar_color, 2);

        // 3. Nom de l'interlocuteur (Speaker Badge)
        let speaker_str = if self.speaker.is_empty() { "INTERLOCUTEUR" } else { &self.speaker };
        let spk_w = (speaker_str.len() * 12 + 20) as u32;
        canvas.draw_badge(card_x + 92, card_y + 14, spk_w, 24, 0x001E293B, 0x00FBBF24, speaker_str, 0x00FDE047, 2);

        // 4. Découpage du texte révélé (machine à écrire)
        let mut byte_limit = self.full_text.len();
        for (i, (b_idx, _)) in self.full_text.char_indices().enumerate() {
            if i == self.revealed_chars {
                byte_limit = b_idx;
                break;
            }
        }
        let visible_str = &self.full_text[..byte_limit];

        // Rendu multi-lignes sans allocation dynamique (word-wrap sur 52 caractères max par ligne)
        let max_line_chars = ((card_w - 110) / 12) as usize;
        let text_start_x = card_x + 96;
        let mut text_cur_y = card_y + 46;

        let words: Vec<&str> = visible_str.split(' ').collect();
        let mut current_line = String::with_capacity(64);

        for word in words {
            if current_line.len() + word.len() + 1 > max_line_chars && !current_line.is_empty() {
                canvas.draw_text(text_start_x, text_cur_y, &current_line, 0x00F8FAFC, 2);
                text_cur_y += 18;
                current_line.clear();
            }
            if !current_line.is_empty() {
                current_line.push(' ');
            }
            current_line.push_str(word);
        }
        if !current_line.is_empty() {
            canvas.draw_text(text_start_x, text_cur_y, &current_line, 0x00F8FAFC, 2);
        }

        // 5. Zone interactive inférieure (Boutons de Choix ou Indicateur Suivant)
        let is_finished_typing = self.revealed_chars >= self.full_text.chars().count();

        if !self.choices.is_empty() {
            let btn_y = card_y + card_h as i32 - 42;
            let btn_h = 30u32;
            let half_w = ((card_w - 110) / 2) as u32;

            // Choix 1
            if let Some(c1) = self.choices.first() {
                let btn_x = card_x + 96;
                let label = format!("[1] {}", c1);
                let col = if is_finished_typing { 0x0038BDF8 } else { 0x00475569 };
                canvas.draw_badge(btn_x, btn_y, half_w, btn_h, 0x001E293B, col, &label, col, 2);
                self.choice_1_hitbox = Some((btn_x, btn_y, half_w, btn_h));
            }

            // Choix 2
            if self.choices.len() >= 2 {
                let btn_x = card_x + 96 + half_w as i32 + 10;
                let label = format!("[2] {}", self.choices[1]);
                let col = if is_finished_typing { 0x00F59E0B } else { 0x00475569 };
                canvas.draw_badge(btn_x, btn_y, half_w, btn_h, 0x001E293B, col, &label, col, 2);
                self.choice_2_hitbox = Some((btn_x, btn_y, half_w, btn_h));
            }
        } else {
            // Bouton de continuation / confirmation
            let btn_w = 260u32;
            let btn_h = 28u32;
            let btn_x = card_x + card_w as i32 - btn_w as i32 - 16;
            let btn_y = card_y + card_h as i32 - 38;

            let prompt_text = if is_finished_typing {
                "[ ESPACE / E : SUIVANT > ]"
            } else {
                "[ CLIC / ESPACE : PASSER ]"
            };
            let prompt_col = if is_finished_typing { 0x0010B981 } else { 0x0064748B };
            canvas.draw_badge(btn_x, btn_y, btn_w, btn_h, 0x001E293B, prompt_col, prompt_text, prompt_col, 2);
            self.advance_btn_hitbox = Some((btn_x, btn_y, btn_w, btn_h));
        }
    }
}

/// Modèle d'affichage du journal et suivi de quêtes HUD
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuestTracker {
    pub id: String,
    pub title: String,
    pub objective: String,
    pub current_step: i64,
    pub max_step: i64,
    pub is_completed: bool,
    pub notification_timer: f32,
}

impl QuestTracker {
    pub fn new(id: impl Into<String>, title: impl Into<String>, objective: impl Into<String>, max_step: i64) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            objective: objective.into(),
            current_step: 0,
            max_step,
            is_completed: false,
            notification_timer: 0.0,
        }
    }

    pub fn set_step(&mut self, step: i64) {
        self.current_step = step;
        self.notification_timer = 3.5;
        if self.current_step >= self.max_step {
            self.is_completed = true;
        }
    }

    pub fn complete(&mut self) {
        self.current_step = self.max_step;
        self.is_completed = true;
        self.notification_timer = 5.0;
    }

    pub fn update(&mut self, dt: f32) {
        self.notification_timer = (self.notification_timer - dt).max(0.0);
    }

    /// Rendu du widget de quête dans le coin supérieur droit sous le bandeau HUD
    pub fn render(&self, canvas: &mut Canvas, x: i32, y: i32) {
        let box_w = 280u32;
        let box_h = 62u32;

        let bg_col = if self.is_completed { 0x00064E3B } else { 0x000F172A };
        let bd_col = if self.is_completed { 0x0010B981 } else { 0x00F59E0B };

        canvas.draw_rect(x, y, box_w, box_h, bg_col);
        canvas.draw_rect_outline(x, y, box_w, box_h, bd_col, 1);
        canvas.draw_corner_brackets(x, y, box_w, box_h, 8, bd_col);

        let title_lbl = format!("QUÊTE : {}", self.title);
        canvas.draw_text(x + 10, y + 8, &title_lbl, bd_col, 2);

        let step_lbl = if self.is_completed {
            "STATUT : TERMINÉE (100%)".to_string()
        } else {
            format!("STATUT : ÉTAPE {} / {}", self.current_step, self.max_step)
        };
        let step_col = if self.is_completed { 0x0034D399 } else { 0x0094A3B8 };
        canvas.draw_text(x + 10, y + 26, &step_lbl, step_col, 2);

        canvas.draw_text(x + 10, y + 42, &self.objective, 0x00CBD5E1, 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dialogue_box_typewriter_and_fast_forward() {
        let mut dlg = DialogueBox::new();
        dlg.open("Gardien", "Bonjour cher aventurier.", vec!["Oui".into(), "Non".into()], "G", 0x0038BDF8);

        assert!(dlg.is_active);
        assert_eq!(dlg.revealed_chars, 0);
        assert!(dlg.is_typing());

        // Accélération instantanée
        dlg.fast_forward();
        assert!(!dlg.is_typing());
        assert_eq!(dlg.revealed_chars, dlg.full_text.chars().count());
    }

    #[test]
    fn test_dialogue_choices_and_close() {
        let mut dlg = DialogueBox::new();
        dlg.open("Sage", "Prenez garde au donjon.", vec!["Entrer".into(), "Fuir".into()], "SAGE", 0x00A855F7);
        dlg.fast_forward();

        let mut input = InputState::new();
        input.on_key_down(winit::keyboard::KeyCode::Digit1);

        let action = dlg.handle_input(&input);
        assert_eq!(action, DialogueAction::ChoiceSelected(1));
        assert_eq!(dlg.selected_choice, Some(1));
        assert!(!dlg.is_active);
    }

    #[test]
    fn test_quest_tracker_progression() {
        let mut quest = QuestTracker::new("quest_01", "La Cle Ancienne", "Trouver les 3 runes", 3);
        assert_eq!(quest.current_step, 0);
        assert!(!quest.is_completed);

        quest.set_step(2);
        assert_eq!(quest.current_step, 2);
        assert!(!quest.is_completed);

        quest.complete();
        assert_eq!(quest.current_step, 3);
        assert!(quest.is_completed);
    }
}
