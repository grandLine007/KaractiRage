use crate::game::GameState;
use crate::input::InputState;
use crate::screens::GameScreen;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use winit::keyboard::KeyCode;

/// Pas de temps fixe (60 Hz) utilisé pour enregistrer puis rejouer un scénario.
///
/// Il est figé dans le module : un replay doit rejouer avec **exactement** le même
/// `dt` que l'enregistrement, sinon les intégrations physiques divergent.
pub const REPLAY_FRAME_DT: f32 = 0.016;

/// Scène de base chargée avant `load_project` (le projet écrase ensuite son contenu).
const REFERENCE_BASE_SCENE: &str = "assets/scenes/level_01.scene.json";

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
    /// Validation d'écran (menu, pause, défaite, victoire) : touche `Entrée`.
    ///
    /// Ajouté après coup sans casser le format : il s'agit d'un nouveau bit du
    /// même `u32`, les sessions existantes restent lisibles (bit à 0).
    pub const KEY_CONFIRM: u32 = 1 << 8;
    /// Pause : touche `Échap`.
    pub const KEY_PAUSE: u32 = 1 << 9;

    /// Table de correspondance entre bits logiques et touches physiques.
    ///
    /// Plusieurs touches peuvent partager un bit (AZERTY/QWERTY) : c'est déjà la
    /// convention de l'enregistrement intégré au moteur, donc la relecture reste
    /// fidèle au niveau du gameplay, qui teste ces touches en `OU`.
    pub const KEY_BINDINGS: &'static [(u32, KeyCode)] = &[
        (Self::KEY_LEFT, KeyCode::ArrowLeft),
        (Self::KEY_LEFT, KeyCode::KeyA),
        (Self::KEY_LEFT, KeyCode::KeyQ),
        (Self::KEY_RIGHT, KeyCode::ArrowRight),
        (Self::KEY_RIGHT, KeyCode::KeyD),
        (Self::KEY_UP, KeyCode::ArrowUp),
        (Self::KEY_UP, KeyCode::KeyW),
        (Self::KEY_UP, KeyCode::KeyZ),
        (Self::KEY_DOWN, KeyCode::ArrowDown),
        (Self::KEY_DOWN, KeyCode::KeyS),
        (Self::KEY_JUMP, KeyCode::Space),
        (Self::KEY_FIRE, KeyCode::KeyF),
        (Self::KEY_INTERACT, KeyCode::KeyE),
        (Self::KEY_RUN, KeyCode::ShiftLeft),
        (Self::KEY_RUN, KeyCode::ShiftRight),
        (Self::KEY_CONFIRM, KeyCode::Enter),
        (Self::KEY_PAUSE, KeyCode::Escape),
    ];

    /// Capture l'état des touches **maintenues** de `input` (pas les transitoires).
    ///
    /// Le couple [`Self::capture`] / [`Self::apply_to`] reconstruit fidèlement les
    /// `just_pressed` : une touche absente d'une frame puis présente à la suivante
    /// produit bien un front montant lors du rejeu.
    pub fn capture(input: &InputState) -> Self {
        let mut snapshot = Self::default();
        for &(mask, key) in Self::KEY_BINDINGS {
            if input.is_key_down(key) {
                snapshot.set_key(mask, true);
            }
        }
        snapshot.mouse_x = input.mouse_x.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        snapshot.mouse_y = input.mouse_y.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        snapshot.mouse_clicked = input.mouse_pressed;
        snapshot
    }

    /// Applique ce snapshot à `input` en générant les fronts montants/descendants.
    ///
    /// À appeler avant chaque `GameState::update`, puis `InputState::clear_transient`.
    pub fn apply_to(&self, input: &mut InputState) {
        for &(mask, key) in Self::KEY_BINDINGS {
            if self.is_pressed(mask) {
                input.on_key_down(key);
            } else {
                input.on_key_up(key);
            }
        }
        input.on_mouse_move(self.mouse_x as f32, self.mouse_y as f32);
        if self.mouse_clicked {
            input.on_mouse_down();
        } else {
            input.on_mouse_up();
        }
    }

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

/// Directive scénarisée rejouée à un index de frame précis.
///
/// Les entrées seules ne suffisent pas à un scénario de test : la traversée du jeu
/// de référence place le joueur devant chaque porte (aucune marche réelle n'est
/// simulée). Cette action est donc **partie intégrante du replay** et rejouée au
/// même moment, ce qui garantit le déterminisme sans toucher à `game.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplayDirective {
    /// Téléporte le joueur à portée d'interaction (< 80 px) de la porte courante.
    TeleportToDoor,
}

impl ReplayDirective {
    fn apply(self, state: &mut GameState) -> Result<(), String> {
        match self {
            Self::TeleportToDoor => teleport_player_to_door(state),
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
    /// Directives scénarisées, indexées par frame (`(index, directive)`).
    ///
    /// Champ optionnel ajouté sans casser la compatibilité : une session
    /// enregistrée par l'ancien format se relit avec un vecteur vide.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scenario_actions: Vec<(usize, ReplayDirective)>,
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
            scenario_actions: Vec::new(),
        });
        self.status_notification = Some("Replay : Enregistrement démarré [REC]".into());
    }

    /// Enregistre une frame à partir de l'état d'entrée brut.
    ///
    /// À appeler **avant** `GameState::update` pour que le rejeu reproduise les
    /// mêmes fronts de touches. `player_pos` alimente le joueur fantôme.
    pub fn record_frame(&mut self, input: &InputState, player_pos: (f32, f32)) {
        self.record_tick(FrameInputSnapshot::capture(input), player_pos);
    }

    /// Enregistre une directive scénarisée à la frame courante (avant son exécution).
    pub fn record_scenario_action(&mut self, directive: ReplayDirective) {
        if self.mode != ReplayMode::Recording {
            return;
        }
        if let Some(ref mut session) = self.active_session {
            let frame = session.frames.len();
            session.scenario_actions.push((frame, directive));
        }
    }

    /// Rejoue la session active sur `state` et vérifie le hash final enregistré.
    ///
    /// `state` doit être une instance **neuve** du même projet (même scène de base,
    /// même `load_project`). Retourne le nombre de frames rejouées.
    pub fn replay_frames(&mut self, state: &mut GameState) -> Result<usize, String> {
        let (frame_count, actions, expected_hash) = match self.active_session.as_ref() {
            Some(session) if !session.frames.is_empty() => (
                session.frames.len(),
                session.scenario_actions.clone(),
                session.final_state_hash,
            ),
            Some(_) => return Err("Replay vide : aucune frame à rejouer".into()),
            None => {
                return Err(
                    "Aucune session de replay : lancez un enregistrement avant de rejouer".into(),
                )
            }
        };

        if !self.start_playback() {
            return Err("Impossible de démarrer la relecture du replay".into());
        }

        let mut input = InputState::new();
        for index in 0..frame_count {
            for (_, directive) in actions.iter().filter(|(frame, _)| *frame == index) {
                if let Err(error) = directive.apply(state) {
                    self.stop_playback();
                    return Err(error);
                }
            }

            let Some(snapshot) = self.next_playback_input() else {
                self.stop_playback();
                return Err(format!(
                    "Replay interrompu prématurément à la frame {index}/{frame_count}"
                ));
            };
            snapshot.apply_to(&mut input);
            state.update(REPLAY_FRAME_DT, &input);
            input.clear_transient();
        }

        self.stop_playback();

        if expected_hash != 0 {
            let actual = state.state_hash();
            if actual != expected_hash {
                return Err(format!(
                    "Divergence de replay : hash final {actual:016x} ≠ {expected_hash:016x} attendu"
                ));
            }
        }
        Ok(frame_count)
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

/// Empreinte déterministe de l'**état de jeu** (pas des allocations mémoire).
///
/// Sérialise une représentation canonique : nom de scène, index de niveau, écran,
/// variables KVL triées par clé, puis chaque entité dans l'ordre du vecteur avec
/// ses positions arrondies au centième. Toute `HashMap` est triée : deux exécutions
/// du même scénario doivent produire exactement la même chaîne, donc le même hash
/// FNV-1a (`ai_dictionary::hash_text`, déjà utilisé pour l'intégrité des assets).
pub fn state_digest(state: &GameState) -> String {
    let mut canonical = String::with_capacity(4096);

    let _ = writeln!(canonical, "scene={}", state.scene_name);
    let _ = writeln!(canonical, "level={}", state.level_index);
    let _ = writeln!(canonical, "screen={:?}", state.screen);

    let mut variables: Vec<_> = state.kvl_context.variables.iter().collect();
    variables.sort_by(|a, b| a.0.cmp(b.0));
    for (name, value) in variables {
        let _ = writeln!(canonical, "var:{name}={value:?}");
    }

    for entity in &state.entities {
        let transform = &entity.transform;
        let _ = writeln!(
            canonical,
            "ent:{}|{}|{}|{:.2}|{:.2}|{:.2}|{:.2}|{}",
            entity.id,
            entity.name,
            entity.tag,
            transform.x,
            transform.y,
            transform.w,
            transform.h,
            entity.is_active
        );
    }

    crate::ai_dictionary::hash_text(canonical.as_bytes())
}

/// Résout un chemin d'asset depuis le répertoire courant ou la racine du dépôt.
///
/// Les tests tournent tantôt depuis la racine du workspace, tantôt depuis le crate :
/// on tente le chemin direct puis le repli `../../`, comme le fait déjà `game.rs`.
fn resolve_project_path(project_path: &str) -> std::path::PathBuf {
    let direct = std::path::Path::new(project_path);
    if direct.exists() {
        return direct.to_path_buf();
    }
    let fallback = std::path::Path::new("../../").join(project_path);
    if fallback.exists() {
        return fallback;
    }
    direct.to_path_buf()
}

/// Instancie une partie neuve du jeu de référence, prête à être enregistrée/rejouée.
fn fresh_reference_state(project_path: &str) -> Result<GameState, String> {
    let resolved = resolve_project_path(project_path);
    let project = crate::project::KaractiProject::load_from_file(&resolved).map_err(|error| {
        format!(
            "Impossible de charger le projet de référence '{}' : {error}",
            resolved.display()
        )
    })?;

    let mut state = GameState::load_from_scene(REFERENCE_BASE_SCENE)
        .map_err(|error| format!("Impossible de charger la scène de base : {error}"))?;
    state.load_project(project);
    Ok(state)
}

/// Place le joueur à portée d'interaction (< 80 px) de la porte du niveau courant.
///
/// Reproduit exactement l'aide de test de `game.rs` : le scénario ne simule pas la
/// marche, il téléporte devant chaque porte avant l'appui sur `E`.
fn teleport_player_to_door(state: &mut GameState) -> Result<(), String> {
    let player = state
        .player_index()
        .ok_or_else(|| "Aucun joueur contrôlable dans la scène courante".to_string())?;
    let (door_x, door_y) = state
        .entities
        .iter()
        .find(|entity| entity.tag == "door")
        .map(|door| (door.transform.center_x(), door.transform.center_y()))
        .ok_or_else(|| "Aucune porte dans la scène courante".to_string())?;

    let x = door_x - 60.0 - state.entities[player].transform.w * 0.5;
    let y = door_y - state.entities[player].transform.h * 0.5;
    let transform = &mut state.entities[player].transform;
    transform.x = x;
    transform.y = y;
    transform.prev_x = x;
    transform.prev_y = y;
    state.rebuild_static_spatial_grid();
    Ok(())
}

/// Score KVL courant (0 si absent ou d'un autre type).
fn current_score(state: &GameState) -> i64 {
    match state.kvl_context.get_var("score") {
        Some(kvl_core::Value::Int(value)) => *value,
        _ => 0,
    }
}

/// Enregistre une frame d'entrée puis avance la simulation d'un pas fixe.
fn record_and_step(state: &mut GameState, recorder: &mut ReplayManager, input: &mut InputState) {
    let player_pos = state
        .player_index()
        .map(|index| {
            let transform = &state.entities[index].transform;
            (transform.x, transform.y)
        })
        .unwrap_or((0.0, 0.0));
    recorder.record_frame(input, player_pos);
    state.update(REPLAY_FRAME_DT, input);
    input.clear_transient();
}

/// Rejoue le scénario de référence : menu → 3 portes → victoire.
///
/// Chaque porte est franchie par une **vraie** interaction (`E`), donc par le graphe
/// KVL `OnDoorInteract` qui pose `level_complete` ; le moteur enchaîne ensuite les
/// niveaux puis affiche la victoire. Une frame neutre relâche `E` entre deux portes
/// afin que la relecture produise bien un nouveau front montant à chaque porte.
fn run_reference_scenario(
    state: &mut GameState,
    recorder: &mut ReplayManager,
) -> Result<(), String> {
    let mut input = InputState::new();

    // 1. Menu → niveau 1 (touche Entrée, un seul pas).
    input.on_key_down(KeyCode::Enter);
    record_and_step(state, recorder, &mut input);
    input.on_key_up(KeyCode::Enter);
    if state.screen != GameScreen::Playing {
        return Err(format!(
            "Le scénario n'a pas quitté le menu (écran : {:?})",
            state.screen
        ));
    }
    if state.level_index != 0 {
        return Err(format!(
            "Le scénario devait démarrer au niveau 1 (index {})",
            state.level_index
        ));
    }

    // 2. Trois portes : une interaction réelle par niveau.
    for level in 0..3 {
        recorder.record_scenario_action(ReplayDirective::TeleportToDoor);
        teleport_player_to_door(state)?;

        input.on_key_up(KeyCode::KeyE);
        input.on_key_down(KeyCode::KeyE);
        // Frame A : `just_pressed(E)` déclenche le graphe KVL de la porte.
        record_and_step(state, recorder, &mut input);
        // Frame B : le moteur consomme `level_complete` (niveau suivant ou victoire).
        record_and_step(state, recorder, &mut input);

        // Frame C neutre : relâche E pour le prochain front montant.
        input.on_key_up(KeyCode::KeyE);
        record_and_step(state, recorder, &mut input);

        match level {
            0 | 1 => {
                if state.level_index != level + 1 {
                    return Err(format!(
                        "Niveau {} non atteint après la porte {} (index {}, écran {:?})",
                        level + 2,
                        level + 1,
                        state.level_index,
                        state.screen
                    ));
                }
                if state.screen != GameScreen::Playing {
                    return Err(format!(
                        "Écran inattendu après la porte {} : {:?}",
                        level + 1,
                        state.screen
                    ));
                }
            }
            _ => {
                if state.screen != GameScreen::Victory {
                    return Err(format!(
                        "Victoire non atteinte après la porte du dernier niveau (écran : {:?})",
                        state.screen
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Enregistre le scénario complet du jeu de référence et retourne le replay + digest.
///
/// Le parcours couvert est celui du critère AC2 : **menu → niveau 1 → niveau 2 →
/// niveau 3 → victoire**, exclusivement par les entrées réelles (`Entrée`, `E`).
/// Le digest final est celui de l'état enregistré ; `replay_to_state` doit le
/// reproduire à l'identique.
pub fn replay_session(project_path: &str) -> Result<(ReplayManager, String), String> {
    let mut state = fresh_reference_state(project_path)?;
    let mut recorder = ReplayManager::new();
    recorder.start_recording("scénario:jeu_reference", state.state_hash());

    run_reference_scenario(&mut state, &mut recorder)?;

    if state.screen != GameScreen::Victory {
        return Err(format!(
            "Le scénario de référence doit se terminer en victoire (écran : {:?})",
            state.screen
        ));
    }

    let digest = state_digest(&state);
    let final_hash = state.state_hash();
    let score = current_score(&state);
    recorder.stop_recording(final_hash, score);
    Ok((recorder, digest))
}

/// Rejoue un replay enregistré sur une **instance neuve** et retourne l'état final.
pub fn replay_to_state(
    project_path: &str,
    replay: &ReplayManager,
) -> Result<GameState, String> {
    let mut state = fresh_reference_state(project_path)?;
    let mut replayer = replay.clone();
    replayer.replay_frames(&mut state)?;
    Ok(state)
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

    /// Chemin du jeu de référence livré (robuste au répertoire courant des tests).
    fn reference_project_path() -> &'static str {
        if std::path::Path::new("assets/projects/jeu_reference.karacti").exists() {
            "assets/projects/jeu_reference.karacti"
        } else {
            "../../assets/projects/jeu_reference.karacti"
        }
    }

    #[test]
    fn captured_frames_preserve_key_transitions() {
        let mut manager = ReplayManager::new();
        manager.start_recording("test", 0);

        let mut input = InputState::new();
        input.on_key_down(KeyCode::Enter);
        manager.record_frame(&input, (0.0, 0.0));
        input.clear_transient();
        manager.record_frame(&input, (0.0, 0.0));
        input.on_key_up(KeyCode::Enter);
        manager.record_frame(&input, (0.0, 0.0));

        let session = manager
            .stop_recording(0, 0)
            .expect("session enregistrée")
            .clone();
        assert_eq!(session.frames.len(), 3);

        let mut replayed = InputState::new();
        session.frames[0].apply_to(&mut replayed);
        assert!(
            replayed.is_key_just_pressed(KeyCode::Enter),
            "Entrée doit produire un front montant à la première frame"
        );
        replayed.clear_transient();

        session.frames[1].apply_to(&mut replayed);
        assert!(replayed.is_key_down(KeyCode::Enter), "Entrée reste maintenue");
        assert!(
            !replayed.is_key_just_pressed(KeyCode::Enter),
            "aucun nouveau front montant sans relâchement"
        );
        replayed.clear_transient();

        session.frames[2].apply_to(&mut replayed);
        assert!(
            !replayed.is_key_down(KeyCode::Enter),
            "Entrée doit être relâchée après le key-up enregistré"
        );
    }

    #[test]
    fn replay_session_json_remains_backward_compatible() {
        // Ancien format : aucun champ `scenario_actions`.
        let legacy = concat!(
            r#"{"scene_path":"assets/scenes/level_01.scene.json","initial_state_hash":0,"#,
            r#""frames":[],"ghost_samples":[],"final_state_hash":0,"total_score":0,"#,
            r#""duration_seconds":0.0}"#
        );
        let session: ReplaySession =
            serde_json::from_str(legacy).expect("une session de l'ancien format reste lisible");
        assert!(
            session.scenario_actions.is_empty(),
            "absence de directives = vecteur vide"
        );

        // Le champ optionnel n'est pas sérialisé quand il est vide : le format
        // courant reste identique à l'ancien pour un replay sans directive.
        let json = serde_json::to_string(&session).expect("session sérialisable");
        assert!(
            !json.contains("scenario_actions"),
            "le format existant ne doit pas être pollué pour un replay simple"
        );
    }

    #[test]
    fn replay_of_reference_game_reproduces_the_same_final_state() {
        let project = reference_project_path();

        // Deux exécutions indépendantes du scénario complet.
        let (recorded, first_digest) =
            replay_session(project).expect("première session de replay enregistrée");
        let (_, second_digest) =
            replay_session(project).expect("seconde session de replay enregistrée");
        assert_eq!(
            first_digest, second_digest,
            "deux exécutions du même scénario doivent produire exactement le même digest"
        );
        assert!(!first_digest.is_empty(), "le digest ne doit pas être vide");

        // Le replay rejoué sur une instance neuve doit converger vers le même état.
        let replayed = replay_to_state(project, &recorded).expect("replay rejoué");
        assert_eq!(
            state_digest(&replayed),
            first_digest,
            "l'état rejoué doit être identique à l'état enregistré"
        );
        assert_eq!(
            replayed.screen,
            GameScreen::Victory,
            "le scénario doit traverser menu → 3 niveaux → victoire"
        );
        assert_eq!(
            replayed.level_index, 2,
            "le dernier niveau (index 2) doit être atteint"
        );

        let session = recorded
            .active_session
            .as_ref()
            .expect("session de replay présente");
        assert_eq!(session.scenario_actions.len(), 3, "une téléportation par porte");
        assert!(!session.frames.is_empty(), "le replay contient des frames");
        assert_eq!(recorded.mode, ReplayMode::Idle);
    }

    #[test]
    fn state_digest_changes_when_the_state_differs() {
        let project = reference_project_path();
        let (recorded, digest) = replay_session(project).expect("session de replay enregistrée");
        let mut state = replay_to_state(project, &recorded).expect("état rejoué");

        assert_eq!(
            state_digest(&state),
            digest,
            "l'empreinte doit être stable pour un état identique"
        );

        // Un déplacement d'un seul pixel doit changer l'empreinte.
        let player = state.player_index().expect("joueur présent");
        state.entities[player].transform.x += 1.0;
        let moved_digest = state_digest(&state);
        assert_ne!(
            moved_digest, digest,
            "un déplacement de 1 px du joueur doit changer l'empreinte"
        );

        // Une variable KVL différente doit aussi changer l'empreinte.
        state
            .kvl_context
            .set_var("score", kvl_core::Value::Int(4242));
        assert_ne!(
            state_digest(&state),
            moved_digest,
            "un changement de variable KVL doit changer l'empreinte"
        );
    }
}
