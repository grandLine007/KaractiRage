//! API native du moteur KaractiRage.
//!
//! Ce module encapsule une instance complète du moteur (`GameState` + `Canvas` +
//! `InputState`) derrière une API simple et **thread-safe** (la structure est
//! `Send`), afin d'être pilotée par le backend Tauri du Studio — un seul moteur,
//! une seule compilation.

use crate::game::GameState;
use crate::input::InputState;
use crate::renderer::Canvas;
use winit::keyboard::KeyCode;

pub const CANVAS_WIDTH: usize = 1200;
pub const CANVAS_HEIGHT: usize = 800;

/// Pas de simulation fixe (120 Hz) pour un déterminisme indépendant du taux de
/// rafraîchissement de l'affichage.
pub const SIM_DT: f32 = 1.0 / 120.0;
/// Nombre maximal de sous-pas de simulation par frame (protection anti "spirale de la mort").
const MAX_SUBSTEPS: u32 = 4;

/// Ventilation du temps CPU d'une frame complète (en microsecondes).
/// Mesuré par `NativeEngine::render_frame` pour alimenter la télémétrie
/// et l'adaptation de qualité avec des valeurs réelles.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameStats {
    pub update_us: u64,
    pub render_us: u64,
    pub encode_us: u64,
    pub total_us: u64,
}

/// Instance embarquée du moteur, découplée de toute fenêtre OS.
pub struct NativeEngine {
    pub game: GameState,
    pub canvas: Canvas,
    pub input: InputState,
    /// Ventilation du temps de la dernière frame rendue.
    pub frame_stats: FrameStats,
    /// Accumulateur de temps pour le pas de simulation fixe.
    accumulator: f32,
}

/// Résout le chemin de la scène par défaut, quel que soit le répertoire courant
/// (lancement depuis la racine du dépôt, ou depuis `apps/studio` via Tauri).
fn locate_scene() -> String {
    let candidates = [
        "assets/scenes/level_01.scene.json".to_string(),
        format!(
            "{}/../../assets/scenes/level_01.scene.json",
            env!("CARGO_MANIFEST_DIR")
        ),
    ];
    for c in candidates {
        if std::path::Path::new(&c).exists() {
            return c;
        }
    }
    "assets/scenes/level_01.scene.json".to_string()
}

fn web_key_to_keycode(key: u32) -> Option<KeyCode> {
    match key {
        0 => Some(KeyCode::ArrowLeft),
        1 => Some(KeyCode::ArrowRight),
        2 => Some(KeyCode::ArrowUp),
        3 => Some(KeyCode::ArrowDown),
        4 => Some(KeyCode::KeyW),
        5 => Some(KeyCode::KeyA),
        6 => Some(KeyCode::KeyS),
        7 => Some(KeyCode::KeyD),
        8 => Some(KeyCode::KeyZ),
        9 => Some(KeyCode::KeyQ),
        10 => Some(KeyCode::KeyE),
        11 => Some(KeyCode::Space),
        12 => Some(KeyCode::Tab),
        13 => Some(KeyCode::KeyN),
        14 => Some(KeyCode::KeyG),
        15 => Some(KeyCode::KeyM),
        16 => Some(KeyCode::F1),
        17 => Some(KeyCode::F2),
        18 => Some(KeyCode::F3),
        19 => Some(KeyCode::KeyR),
        20 => Some(KeyCode::Escape),
        21 => Some(KeyCode::ControlLeft),
        22 => Some(KeyCode::KeyV),
        23 => Some(KeyCode::KeyB),
        24 => Some(KeyCode::KeyX),
        25 => Some(KeyCode::KeyT),
        26 => Some(KeyCode::KeyP),
        27 => Some(KeyCode::KeyH),
        28 => Some(KeyCode::KeyU),
        29 => Some(KeyCode::F4),
        30 => Some(KeyCode::Digit1),
        31 => Some(KeyCode::Digit2),
        32 => Some(KeyCode::Digit3),
        33 => Some(KeyCode::Digit4),
        34 => Some(KeyCode::Digit5),
        35 => Some(KeyCode::KeyI),
        36 => Some(KeyCode::Enter),
        _ => None,
    }
}

/// Échange les canaux rouge et bleu d'un pixel `0x00RRGGBB` (XRGB softbuffer)
/// pour produire un pixel `0xAABBGGRR` dont l'ordre mémoire little-endian est
/// exactement (R, G, B, A) — prêt à être copié vers un `<canvas>` HTML5.
#[inline]
fn swizzle_xrgb_to_rgba(px: u32) -> u32 {
    0xFF00_0000 | ((px & 0x0000_00FF) << 16) | (px & 0x0000_FF00) | ((px & 0x00FF_0000) >> 16)
}

impl NativeEngine {
    /// Initialise une instance du moteur avec la scène par défaut.
    pub fn init() -> Result<Self, String> {
        let path = locate_scene();
        let mut game = GameState::load_from_scene(&path)
            .map_err(|e| format!("Impossible de charger la scène '{}': {}", path, e))?;
        game.editor.embedded_mode = true;
        game.editor.show_welcome_modal = false;
        game.editor.show_help_modal = false;

        let canvas = Canvas::new(CANVAS_WIDTH, CANVAS_HEIGHT);
        crate::renderer::warmup_glyph_cache();
        let input = InputState::new();

        Ok(Self {
            game,
            canvas,
            input,
            frame_stats: FrameStats::default(),
            accumulator: 0.0,
        })
    }

    pub fn canvas_width(&self) -> u32 {
        CANVAS_WIDTH as u32
    }

    pub fn canvas_height(&self) -> u32 {
        CANVAS_HEIGHT as u32
    }

    /// Avance la simulation par pas fixes (120 Hz), génère le rendu logiciel,
    /// encode le framebuffer en octets RGBA, puis renseigne la télémétrie complète.
    pub fn render_frame(&mut self, dt: f32) -> Vec<u8> {
        // Clamp défensif : un très long gel d'UI ne doit pas provoquer une
        // avalanche de sous-pas (spirale de la mort).
        self.accumulator += dt.clamp(0.0, 0.25);

        let t0 = std::time::Instant::now();
        let mut steps = 0u32;
        while self.accumulator >= SIM_DT && steps < MAX_SUBSTEPS {
            self.game.update(SIM_DT, &self.input);
            self.accumulator -= SIM_DT;
            steps += 1;
            // Les entrées transitoires (just_pressed / click) ne doivent être
            // consommées qu'une seule fois par frame, pas à chaque sous-pas.
            if steps == 1 {
                self.input.clear_transient();
            }
        }
        let t1 = std::time::Instant::now();
        self.game.render(&mut self.canvas);
        let t2 = std::time::Instant::now();
        // Sécurité : garantit que les transitoires sont purgées même sans sous-pas.
        self.input.clear_transient();
        let bytes = self.frame_bytes();
        let t3 = std::time::Instant::now();

        let update_us = (t1 - t0).as_micros() as u64;
        let render_us = (t2 - t1).as_micros() as u64;
        let encode_us = (t3 - t2).as_micros() as u64;
        let total_us = update_us + render_us + encode_us;
        self.frame_stats = FrameStats {
            update_us,
            render_us,
            encode_us,
            total_us,
        };
        self.game.finish_frame_telemetry(total_us);
        bytes
    }

    /// Encode le framebuffer courant en octets RGBA (R, G, B, A par pixel),
    /// en une seule passe : échange R/B + écriture directe des 4 octets du pixel.
    pub fn frame_bytes(&self) -> Vec<u8> {
        let n = self.canvas.buffer.len();
        if n == 0 {
            return Vec::new();
        }

        let mut out = Vec::with_capacity(n * 4);
        // Le tampon alloué par `Vec<u8>` est aligné pour `u32` (≥ 4 octets) ;
        // on le réinterprète en tranche de `u32` pour écrire 4 octets par store.
        let out_u32: &mut [u32] =
            unsafe { std::slice::from_raw_parts_mut(out.as_mut_ptr() as *mut u32, n) };
        for (i, px) in self.canvas.buffer.iter().enumerate() {
            out_u32[i] = swizzle_xrgb_to_rgba(*px);
        }
        // Safety: les `n` éléments u32 (soit `n * 4` octets) ont tous été écrits.
        unsafe { out.set_len(n * 4) };
        out
    }

    // --- Entrées ---------------------------------------------------------------

    pub fn on_key_down(&mut self, key: u32) {
        if let Some(kc) = web_key_to_keycode(key) {
            self.input.on_key_down(kc);
        }
    }

    pub fn on_key_up(&mut self, key: u32) {
        if let Some(kc) = web_key_to_keycode(key) {
            self.input.on_key_up(kc);
        }
    }

    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        self.input.on_mouse_move(x, y);
    }

    pub fn on_mouse_down(&mut self, button: u32) {
        if button == 0 {
            self.input.on_mouse_down();
        } else if button == 2 {
            self.input.on_mouse_right_click();
        }
    }

    pub fn on_mouse_up(&mut self, button: u32) {
        if button == 0 {
            self.input.on_mouse_up();
        }
    }

    // --- Télémétrie -----------------------------------------------------------

    pub fn get_score(&self) -> i32 {
        self.game
            .kvl_context
            .get_var("score")
            .map_or(0, |v| match v {
                kvl_core::Value::Int(s) => *s as i32,
                _ => 0,
            })
    }

    pub fn get_fps(&self) -> f32 {
        self.game.telemetry.current_fps
    }

    pub fn get_cpu_us(&self) -> u32 {
        self.game.telemetry.avg_frame_time_us as u32
    }

    pub fn get_spatial_queries(&self) -> u32 {
        self.game.spatial_grid.stats_queries as u32
    }

    pub fn get_status_message(&self) -> String {
        self.game.status_message.clone()
    }

    // --- Bascules -------------------------------------------------------------

    pub fn toggle_radar(&mut self) -> u32 {
        self.game.show_minimap = !self.game.show_minimap;
        if self.game.show_minimap {
            1
        } else {
            0
        }
    }

    pub fn toggle_spatial_grid(&mut self) -> u32 {
        if self.game.spatial_grid.toggle_debug() {
            1
        } else {
            0
        }
    }

    pub fn toggle_editor_mode(&mut self) -> u32 {
        self.game.editor.toggle_mode();
        match self.game.editor.mode {
            crate::editor::EditorMode::Edit => 1,
            crate::editor::EditorMode::Play => 0,
        }
    }

    pub fn set_embedded_mode(&mut self, enabled: bool) {
        self.game.editor.embedded_mode = enabled;
    }

    // --- Export / import ------------------------------------------------------

    pub fn export_standalone_html(&self, title: &str, author: &str) -> String {
        crate::exporter::StandaloneExporter::export_html(&self.game, title, author)
    }

    pub fn export_game_package(&self, title: &str, author: &str) -> String {
        crate::exporter::StandaloneExporter::export_package_json(&self.game, title, author)
            .unwrap_or_else(|_| "{}".to_string())
    }

    pub fn import_game_package(&mut self, package_json: &str) -> i32 {
        let package = match crate::exporter::KaractiGamePackage::from_json(package_json) {
            Ok(p) => p,
            Err(_) => return -1,
        };
        crate::exporter::StandaloneExporter::import_package(&mut self.game, &package);
        0
    }

    // --- Recettes --------------------------------------------------------------

    pub fn validate_recipe(recipe_json: &str) -> String {
        let mut recipe = match crate::recipe::KaractiGameRecipe::from_json(recipe_json) {
            Ok(r) => r,
            Err(e) => {
                return format!("{{\"is_valid\":false,\"conflicts\":[\"JSON invalide: {}\"]}}", e)
            }
        };
        let report = crate::recipe::DependencyResolver::resolve(&mut recipe);
        serde_json::to_string(&report).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn apply_recipe(&mut self, recipe_json: &str) -> i32 {
        let mut recipe = match crate::recipe::KaractiGameRecipe::from_json(recipe_json) {
            Ok(r) => r,
            Err(_) => return -1,
        };
        let report = crate::recipe::DependencyResolver::resolve(&mut recipe);
        if !report.is_valid {
            return -2;
        }
        let (scene, kvl_graphs) = crate::recipe::RecipeProjectGenerator::generate(&recipe);
        self.game.scene_name = scene.name;
        self.game.entities = scene.entities;
        self.game.kvl_graphs = kvl_graphs;
        self.game.editor.selected = crate::editor::SelectedEntity::None;
        self.game.editor.mode = crate::editor::EditorMode::Play;
        self.game
            .set_status(&format!("Recette '{}' instanciée avec succès !", recipe.title));
        0
    }

    // --- IA --------------------------------------------------------------------

    pub fn apply_ai_payload(&mut self, payload_json: &str, prompt: &str) -> i32 {
        let payload: crate::ai_client::AiGeneratedGamePayload =
            match serde_json::from_str(payload_json) {
                Ok(p) => p,
                Err(_) => return -1,
            };
        self.game.apply_ai_payload(payload, prompt);
        0
    }

    pub fn accept_ai_patch(&mut self) -> i32 {
        self.game.accept_ai_patch();
        0
    }

    pub fn reject_ai_patch(&mut self) -> i32 {
        self.game.reject_ai_patch();
        0
    }

    pub fn has_pending_ai_patch(&self) -> i32 {
        if self.game.editor.pending_ai_patch.is_some() {
            1
        } else {
            0
        }
    }

    // --- Outils Studio ---------------------------------------------------------

    pub fn set_active_tool(&mut self, tool: u32) -> u32 {
        let tool_enum = match tool {
            0 => crate::editor::StudioTool::Select,
            1 => crate::editor::StudioTool::Brush,
            2 => crate::editor::StudioTool::Eraser,
            _ => crate::editor::StudioTool::Select,
        };
        self.game.editor.set_tool(tool_enum);
        tool
    }

    pub fn get_active_tool(&self) -> u32 {
        match self.game.editor.active_tool {
            crate::editor::StudioTool::Select => 0,
            crate::editor::StudioTool::Brush => 1,
            crate::editor::StudioTool::Eraser => 2,
        }
    }

    pub fn cycle_brush_tile(&mut self) -> String {
        self.game.editor.cycle_brush_tile().label().to_string()
    }

    pub fn get_active_brush_tile(&self) -> String {
        self.game.editor.brush_tile.label().to_string()
    }

    // --- Navigation / interaction ----------------------------------------------

    pub fn move_player_to(&mut self, target_x: f32, target_y: f32) -> bool {
        if let Some(p_idx) = self.game.player_index() {
            let mut nav = crate::navigation::NavAgent::go_to(260.0, target_x, target_y);
            nav.reach_distance = 18.0;
            nav.avoid_hazards = true;
            self.game.entities[p_idx].nav_agent = Some(nav);
            self.game.status_message = format!(
                "Navigation A* vers ({:.0}, {:.0}) enclenchée.",
                target_x, target_y
            );
            return true;
        }
        false
    }

    pub fn get_player_pos(&self) -> (f32, f32) {
        if let Some(p_idx) = self.game.player_index() {
            (
                self.game.entities[p_idx].transform.x,
                self.game.entities[p_idx].transform.y,
            )
        } else {
            (0.0, 0.0)
        }
    }

    pub fn interact(&mut self) -> bool {
        self.input.on_key_down(KeyCode::KeyE);
        true
    }

    // --- Projets ---------------------------------------------------------------

    pub fn new_blank_project(&mut self, title: &str, genre_str: &str) -> i32 {
        let genre = match genre_str {
            "platformer" => crate::recipe::GameGenre::Platformer,
            "shoot_em_up" | "shmup" => crate::recipe::GameGenre::ShootEmUp,
            "puzzle" | "sokoban" => crate::recipe::GameGenre::Puzzle,
            "combat" | "boss" => crate::recipe::GameGenre::Combat,
            _ => crate::recipe::GameGenre::Adventure,
        };
        self.game.new_blank_project(title, genre);
        0
    }

    pub fn load_project_json(&mut self, project_json: &str) -> i32 {
        let proj = match crate::project::KaractiProject::from_json(project_json) {
            Ok(p) => p,
            Err(_) => return -2,
        };
        self.game.load_project(proj);
        0
    }

    pub fn export_project_json(&self) -> String {
        self.game
            .export_current_project()
            .to_json()
            .unwrap_or_else(|_| "{}".to_string())
    }

    pub fn get_entities_summary_json(&self) -> String {
        self.game.get_entities_summary_json()
    }

    pub fn update_entity_transform(&mut self, id: usize, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.game.update_entity_transform_by_id(id, x, y, w, h)
    }

    pub fn delete_entity(&mut self, id: usize) -> bool {
        self.game.delete_entity_by_id(id)
    }

    /// Instancie une entité selon un préfabriqué (coin, potion, key, chest, door,
    /// platform, wall, spikes, boss, crate, plate).
    pub fn spawn_entity_preset(&mut self, preset: &str, x: f32, y: f32) -> usize {
        let next_id = self.game.entities.iter().map(|e| e.id).max().unwrap_or(0) + 1;
        let ent = match preset {
            "coin" | "collectible" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Piece_{:02}", next_id),
                    "collectible",
                    crate::entity::Transform2D::new(x, y, 24.0, 24.0),
                );
                e.collider = Some(crate::entity::Collider2D::trigger(24.0, 24.0));
                e
            }
            "potion" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Potion_{:02}", next_id),
                    "item",
                    crate::entity::Transform2D::new(x, y, 24.0, 24.0),
                );
                e.collider = Some(crate::entity::Collider2D::trigger(24.0, 24.0));
                e
            }
            "key" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Cle_{:02}", next_id),
                    "item",
                    crate::entity::Transform2D::new(x, y, 24.0, 24.0),
                );
                e.collider = Some(crate::entity::Collider2D::trigger(24.0, 24.0));
                e
            }
            "chest" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Coffre_{:02}", next_id),
                    "item",
                    crate::entity::Transform2D::new(x, y, 36.0, 36.0),
                );
                e.collider = Some(crate::entity::Collider2D::solid(36.0, 36.0));
                e
            }
            "door" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Porte_{:02}", next_id),
                    "door",
                    crate::entity::Transform2D::new(x, y, 32.0, 64.0),
                );
                e.collider = Some(crate::entity::Collider2D::solid(32.0, 64.0));
                e
            }
            "platform" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Plateforme_{:02}", next_id),
                    "platform",
                    crate::entity::Transform2D::new(x, y, 160.0, 24.0),
                );
                e.collider = Some(crate::entity::Collider2D::solid(160.0, 24.0));
                e
            }
            "spikes" | "hazard" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Piques_{:02}", next_id),
                    "hazard",
                    crate::entity::Transform2D::new(x, y, 96.0, 20.0),
                );
                e.collider = Some(crate::entity::Collider2D::trigger(96.0, 20.0));
                e
            }
            "crate" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Caisse_{:02}", next_id),
                    "crate",
                    crate::entity::Transform2D::new(x, y, 32.0, 32.0),
                );
                let mut c = crate::entity::Collider2D::solid(32.0, 32.0);
                c.is_pushable = true;
                e.collider = Some(c);
                e
            }
            "plate" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Dalle_{:02}", next_id),
                    "plate",
                    crate::entity::Transform2D::new(x, y, 32.0, 32.0),
                );
                e.collider = Some(crate::entity::Collider2D::trigger(32.0, 32.0));
                e
            }
            "boss" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Boss_{:02}", next_id),
                    "boss",
                    crate::entity::Transform2D::new(x, y, 72.0, 72.0),
                );
                e.collider = Some(crate::entity::Collider2D::solid(72.0, 72.0));
                e.health = Some(crate::entity::Health2D::new(100.0));
                e
            }
            _ => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Mur_{:02}", next_id),
                    "wall",
                    crate::entity::Transform2D::new(x, y, 32.0, 32.0),
                );
                e.collider = Some(crate::entity::Collider2D::solid(32.0, 32.0));
                e
            }
        };
        self.game.entities.push(ent);
        self.game
            .set_status(format!("Entité #{} ('{}') générée avec succès.", next_id, preset));
        next_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swizzle_xrgb_to_rgba() {
        // 0x00RRGGBB -> 0xAABBGGRR (ordre mémoire R, G, B, A)
        assert_eq!(swizzle_xrgb_to_rgba(0x00FF0000), 0xFF0000FF); // rouge
        assert_eq!(swizzle_xrgb_to_rgba(0x0000FF00), 0xFF00FF00); // vert
        assert_eq!(swizzle_xrgb_to_rgba(0x000000FF), 0xFFFF0000); // bleu
        assert_eq!(swizzle_xrgb_to_rgba(0x00000000), 0xFF000000); // noir opaque
    }

    #[test]
    fn test_frame_bytes_encodes_rgba_order() {
        let mut engine = NativeEngine::init().expect("init moteur");
        // Pixel unique : noir opaque, donc RGBA = [0, 0, 0, 255]
        let n = engine.canvas.buffer.len();
        engine.canvas.buffer[0] = 0x00000000; // XRGB noir
        let bytes = engine.frame_bytes();
        assert_eq!(bytes.len(), n * 4);
        assert_eq!(&bytes[0..4], &[0, 0, 0, 255]);

        // Rouge pur XRGB 0x00FF0000 -> RGBA [255, 0, 0, 255]
        engine.canvas.buffer[0] = 0x00FF0000;
        let bytes = engine.frame_bytes();
        assert_eq!(&bytes[0..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn test_fixed_timestep_clamps_substeps() {
        let mut engine = NativeEngine::init().expect("init moteur");

        // Scénario 1 : un énorme dt (0.5 s) est clampé à 0.25 s dans l'accumulateur,
        // soit 30 pas de 120 Hz, mais plafonné à MAX_SUBSTEPS (4).
        engine.accumulator = 0.0;
        let before = engine.game.sim_tick;
        engine.render_frame(0.5);
        let steps = engine.game.sim_tick - before;
        assert_eq!(steps, 4u64, "un grand dt doit être plafonné à 4 sous-pas");

        // Scénario 2 : un dt nominal (1/60 s) doit produire exactement 2 sous-pas.
        engine.accumulator = 0.0;
        let before = engine.game.sim_tick;
        engine.render_frame(1.0 / 60.0);
        let steps = engine.game.sim_tick - before;
        assert_eq!(steps, 2u64, "1/60 s = 2 sous-pas de 1/120 s");
    }
}
