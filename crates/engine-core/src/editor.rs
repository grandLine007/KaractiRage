use crate::entity::Entity;
use crate::recipe::{
    GameGenre, GameDimension, CameraType, MovementDirections, CombatStyle,
    InteractiveElement, PerformanceProfile, KaractiGameRecipe, RecipeValidationReport,
    DependencyResolver
};

/// Commande réversible pour le système d'historique Undo/Redo du Studio (Command Pattern)
#[derive(Debug, Clone, PartialEq)]
pub enum StudioCommand {
    /// Création ou peinture d'une tuile/entité
    SpawnEntity {
        entity_id: usize,
        entity_name: String,
        cell: Option<(i32, i32)>,
    },
    /// Suppression d'une entité
    DeleteEntity {
        entity: Entity,
        index: usize,
    },
    /// Déplacement spatial d'une entité
    #[allow(dead_code)]
    MoveEntity {
        entity_id: usize,
        old_x: f32,
        old_y: f32,
        new_x: f32,
        new_y: f32,
    },
    ApplyAiPatch {
        patch_id: String,
        description: String,
        created_entity_ids: Vec<usize>,
        created_entities: Vec<Entity>,
        target_graph_id: String,
        previous_graph: Option<kvl_core::Graph>,
        patched_graph: kvl_core::Graph,
    },
}

impl StudioCommand {
    pub fn description(&self) -> String {
        match self {
            StudioCommand::SpawnEntity { entity_name, .. } => format!("Création '{}'", entity_name),
            StudioCommand::DeleteEntity { entity, .. } => format!("Suppression '{}'", entity.name),
            StudioCommand::MoveEntity { entity_id, old_x, old_y, new_x, new_y } => {
                format!("Déplacement #{} ({:.0},{:.0}) -> ({:.0},{:.0})", entity_id, old_x, old_y, new_x, new_y)
            }
            StudioCommand::ApplyAiPatch { description, .. } => format!("Patch IA '{}'", description),
        }
    }
}

/// Pile d'historique Undo / Redo avec capacité bornée sans allocation intempestive
#[derive(Debug, Clone)]
pub struct UndoStack {
    pub undo_list: Vec<StudioCommand>,
    pub redo_list: Vec<StudioCommand>,
    pub max_capacity: usize,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new(50)
    }
}

impl UndoStack {
    pub fn new(capacity: usize) -> Self {
        Self {
            undo_list: Vec::with_capacity(capacity),
            redo_list: Vec::with_capacity(capacity),
            max_capacity: capacity,
        }
    }

    pub fn push(&mut self, cmd: StudioCommand) {
        if self.undo_list.len() >= self.max_capacity {
            self.undo_list.remove(0);
        }
        self.undo_list.push(cmd);
        self.redo_list.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_list.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_list.is_empty()
    }

    pub fn pop_undo(&mut self) -> Option<StudioCommand> {
        self.undo_list.pop()
    }

    pub fn push_redo(&mut self, cmd: StudioCommand) {
        if self.redo_list.len() >= self.max_capacity {
            self.redo_list.remove(0);
        }
        self.redo_list.push(cmd);
    }

    pub fn pop_redo(&mut self) -> Option<StudioCommand> {
        self.redo_list.pop()
    }

    pub fn push_undo(&mut self, cmd: StudioCommand) {
        if self.undo_list.len() >= self.max_capacity {
            self.undo_list.remove(0);
        }
        self.undo_list.push(cmd);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Play,
    Edit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudioTool {
    Select,
    Brush,
    Eraser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushTile {
    Wall,
    Hazard,
    Collectible,
    Heart,
    Crate,
    Plate,
}

impl BrushTile {
    pub fn id(&self) -> &'static str {
        match self {
            BrushTile::Wall => "wall",
            BrushTile::Hazard => "hazard",
            BrushTile::Collectible => "collectible",
            BrushTile::Heart => "heart",
            BrushTile::Crate => "crate",
            BrushTile::Plate => "plate",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            BrushTile::Wall => "Mur Solide",
            BrushTile::Hazard => "Piege Piques",
            BrushTile::Collectible => "Piece KVL",
            BrushTile::Heart => "Fiole Coeur",
            BrushTile::Crate => "Bloc Poussable",
            BrushTile::Plate => "Dalle Pression",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            BrushTile::Wall => "Mur",
            BrushTile::Hazard => "Piege",
            BrushTile::Collectible => "Piece",
            BrushTile::Heart => "Coeur",
            BrushTile::Crate => "Caisse",
            BrushTile::Plate => "Dalle",
        }
    }

    pub fn color(&self) -> u32 {
        match self {
            BrushTile::Wall => 0x0094A3B8,
            BrushTile::Hazard => 0x00EF4444,
            BrushTile::Collectible => 0x00F59E0B,
            BrushTile::Heart => 0x00EC4899,
            BrushTile::Crate => 0x00F4AF6A,
            BrushTile::Plate => 0x0034D399,
        }
    }

    pub fn next(&self) -> Self {
        match self {
            BrushTile::Wall => BrushTile::Hazard,
            BrushTile::Hazard => BrushTile::Collectible,
            BrushTile::Collectible => BrushTile::Heart,
            BrushTile::Heart => BrushTile::Crate,
            BrushTile::Crate => BrushTile::Plate,
            BrushTile::Plate => BrushTile::Wall,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedEntity {
    None,
    Entity(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveWire {
    pub from_node: String,
    pub from_pin: String,
    pub start_x: i32,
    pub start_y: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PinHitbox {
    pub node_id: String,
    pub pin_id: String,
    pub is_output: bool,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub socket_x: i32,
    pub socket_y: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionHitbox {
    pub connection_id: String,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaletteItemHitbox {
    pub template_id: String,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

pub struct EditorState {
    pub mode: EditorMode,
    pub selected: SelectedEntity,
    pub active_tool: StudioTool,
    pub brush_tile: BrushTile,
    pub last_painted_cell: Option<(i32, i32)>,
    pub is_detailed_view: bool,
    pub is_dragging: bool,
    pub drag_offset_x: f32,
    pub drag_offset_y: f32,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub active_wire: Option<ActiveWire>,
    pub pin_hitboxes: Vec<PinHitbox>,
    pub connection_hitboxes: Vec<ConnectionHitbox>,
    pub show_node_palette: bool,
    pub palette_hitboxes: Vec<PaletteItemHitbox>,
    pub save_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub top_save_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub add_node_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub view_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub show_spawn_palette: bool,
    pub spawn_palette_hitboxes: Vec<PaletteItemHitbox>,
    pub spawn_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub duplicate_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub delete_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub tool_select_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub tool_brush_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub tool_eraser_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub tool_tile_cycle_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub add_comp_hitboxes: Vec<(String, i32, i32, u32, u32)>,
    pub show_help_modal: bool,
    pub help_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub mode_toggle_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub genre_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub radar_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub grid_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub telemetry_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub audio_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub reset_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub show_console_drawer: bool,
    pub console_toggle_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub play_tab_hitbox: Option<(i32, i32, u32, u32)>,
    pub studio_tab_hitbox: Option<(i32, i32, u32, u32)>,
    pub wizard: GuidedWizardState,
    pub guided_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub undo_stack: UndoStack,
    pub undo_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub redo_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub active_tooltip: Option<(String, String, String, i32, i32)>,
    pub show_welcome_modal: bool,
    pub welcome_tab: usize,
    pub welcome_tab_hitboxes: Vec<(usize, i32, i32, u32, u32)>,
    pub welcome_scene_hitboxes: Vec<(usize, bool, i32, i32, u32, u32)>,
    pub welcome_archetype_hitboxes: Vec<(&'static str, i32, i32, u32, u32)>,
    pub welcome_play_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub welcome_studio_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub welcome_guide_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub welcome_recipe_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub welcome_close_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub hub_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub guide_tab: usize,
    pub guide_tab_btn_hitboxes: Vec<(usize, i32, i32, u32, u32)>,
    pub guide_step_done: [bool; 6],
    pub guide_close_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub show_intent_bar: bool,
    #[allow(dead_code)]
    pub intent_input_text: String,
    pub intent_bar_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub intent_preset_hitboxes: Vec<(usize, i32, i32, u32, u32)>,
    pub intent_close_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub pending_ai_patch: Option<crate::ai_patch::AiPatch>,
    pub ai_patch_accept_hitbox: Option<(i32, i32, u32, u32)>,
    pub ai_patch_modify_hitbox: Option<(i32, i32, u32, u32)>,
    pub ai_patch_reject_hitbox: Option<(i32, i32, u32, u32)>,
    pub ai_patch_history: Vec<crate::ai_patch::AiPatchHistoryItem>,
    pub intent_submit_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub ai_config: crate::ai_client::AiClientConfig,
    pub ai_provider_btn_hitboxes: Vec<(crate::ai_client::AiProvider, i32, i32, u32, u32)>,
    pub ai_generation_in_progress: bool,
    pub ai_status_text: String,
    pub ai_test_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub ai_test_result: Option<(bool, String)>,
    pub show_export_modal: bool,
    pub export_title: String,
    pub export_author: String,
    pub export_format_idx: usize,
    pub export_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub export_action_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub export_format_hitboxes: Vec<(usize, i32, i32, u32, u32)>,
    pub export_close_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub recipe_wizard: GameRecipeWizardState,
    pub recipe_btn_hitbox: Option<(i32, i32, u32, u32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardCategory {
    Chest,       // 📦 Coffre & Clé
    Door,        // 🚪 Porte ou Barrière
    Hazard,      // ⚠️ Piège & Dégâts
    Teleport,    // 🌀 Portail de Téléportation
    SwitchPlate, // ⚡ Dalle Pression / Énigme
    Collectible, // 🪙 Collecte / Or
}

impl WizardCategory {
    pub fn all() -> &'static [WizardCategory] {
        &[
            WizardCategory::Chest,
            WizardCategory::Door,
            WizardCategory::Hazard,
            WizardCategory::Teleport,
            WizardCategory::SwitchPlate,
            WizardCategory::Collectible,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            WizardCategory::Chest => "Coffre au Tresor & Cle",
            WizardCategory::Door => "Porte ou Barriere Securisee",
            WizardCategory::Hazard => "Zone de Danger / Piege",
            WizardCategory::Teleport => "Portail de Teleportation",
            WizardCategory::SwitchPlate => "Dalle Pression / Interrupteur",
            WizardCategory::Collectible => "Objet a Collecter (Or / Fiole)",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            WizardCategory::Chest => "[COFFRE]",
            WizardCategory::Door => "[PORTE]",
            WizardCategory::Hazard => "[PIEGE]",
            WizardCategory::Teleport => "[PORTAIL]",
            WizardCategory::SwitchPlate => "[DALLE]",
            WizardCategory::Collectible => "[TRESOR]",
        }
    }

    pub fn desc(&self) -> &'static str {
        match self {
            WizardCategory::Chest => "Interaction [E] -> Verifie si le joueur a la cle -> Donne tresor et potion.",
            WizardCategory::Door => "Interaction [E] -> Verifie score ou cle -> Deverrouille la porte.",
            WizardCategory::Hazard => "Contact Joueur -> Inflige des degats instantanes et feedback sonore.",
            WizardCategory::Teleport => "Contact Joueur -> Teleporte vers une position cible avec effet visuel.",
            WizardCategory::SwitchPlate => "Poids pose (caisse ou joueur) -> Active la variable de porte ou l'enigme.",
            WizardCategory::Collectible => "Contact Joueur -> Ajoute des points de score, son et disparition.",
        }
    }

    pub fn color(&self) -> u32 {
        match self {
            WizardCategory::Chest => 0x00F59E0B,     // Or
            WizardCategory::Door => 0x0038BDF8,      // Cyan
            WizardCategory::Hazard => 0x00EF4444,    // Rouge
            WizardCategory::Teleport => 0x00A855F7,  // Violet
            WizardCategory::SwitchPlate => 0x0010B981, // Émeraude
            WizardCategory::Collectible => 0x00FBBF24, // Jaune
        }
    }
}

#[derive(Debug, Clone)]
pub struct GuidedWizardState {
    pub is_open: bool,
    pub step: u32, // 1: Choix d'intention, 2: Questions/Paramètres, 3: Synthèse & Validation
    pub selected_category: WizardCategory,
    pub condition_choice: usize,
    pub action_choice: usize,
    pub sound_choice: usize,

    // Hitboxes
    pub category_hitboxes: Vec<(WizardCategory, i32, i32, u32, u32)>,
    pub condition_option_hitboxes: Vec<(usize, i32, i32, u32, u32)>,
    pub action_option_hitboxes: Vec<(usize, i32, i32, u32, u32)>,
    pub sound_option_hitboxes: Vec<(usize, i32, i32, u32, u32)>,
    pub next_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub prev_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub generate_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub close_btn_hitbox: Option<(i32, i32, u32, u32)>,
}

impl GuidedWizardState {
    pub fn new() -> Self {
        Self {
            is_open: false,
            step: 1,
            selected_category: WizardCategory::Chest,
            condition_choice: 0,
            action_choice: 0,
            sound_choice: 0,
            category_hitboxes: Vec::new(),
            condition_option_hitboxes: Vec::new(),
            action_option_hitboxes: Vec::new(),
            sound_option_hitboxes: Vec::new(),
            next_btn_hitbox: None,
            prev_btn_hitbox: None,
            generate_btn_hitbox: None,
            close_btn_hitbox: None,
        }
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.step = 1;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn next_step(&mut self) {
        if self.step < 3 {
            self.step += 1;
        }
    }

    pub fn prev_step(&mut self) {
        if self.step > 1 {
            self.step -= 1;
        }
    }

    pub fn is_close_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.close_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_next_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.next_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_prev_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.prev_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_generate_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.generate_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn clicked_category(&self, mx: f32, my: f32) -> Option<WizardCategory> {
        for (cat, bx, by, bw, bh) in &self.category_hitboxes {
            if mx >= *bx as f32 && mx <= (*bx + *bw as i32) as f32 && my >= *by as f32 && my <= (*by + *bh as i32) as f32 {
                return Some(*cat);
            }
        }
        None
    }

    pub fn clicked_condition_option(&self, mx: f32, my: f32) -> Option<usize> {
        for (idx, bx, by, bw, bh) in &self.condition_option_hitboxes {
            if mx >= *bx as f32 && mx <= (*bx + *bw as i32) as f32 && my >= *by as f32 && my <= (*by + *bh as i32) as f32 {
                return Some(*idx);
            }
        }
        None
    }

    pub fn clicked_action_option(&self, mx: f32, my: f32) -> Option<usize> {
        for (idx, bx, by, bw, bh) in &self.action_option_hitboxes {
            if mx >= *bx as f32 && mx <= (*bx + *bw as i32) as f32 && my >= *by as f32 && my <= (*by + *bh as i32) as f32 {
                return Some(*idx);
            }
        }
        None
    }

    pub fn clicked_sound_option(&self, mx: f32, my: f32) -> Option<usize> {
        for (idx, bx, by, bw, bh) in &self.sound_option_hitboxes {
            if mx >= *bx as f32 && mx <= (*bx + *bw as i32) as f32 && my >= *by as f32 && my <= (*by + *bh as i32) as f32 {
                return Some(*idx);
            }
        }
        None
    }
}

/// État de l'Assistant de Recette de Jeu en 6 Étapes (Desktop Native Game Recipe Wizard)
#[derive(Debug, Clone)]
pub struct GameRecipeWizardState {
    pub is_open: bool,
    pub step: usize, // 1: Genre, 2: Univers, 3: Joueur, 4: Monde, 5: Gameplay, 6: Validation & Audit
    pub recipe: KaractiGameRecipe,
    pub validation_report: Option<RecipeValidationReport>,
    pub selected_archetype: Option<&'static str>,

    // Hitboxes interactives pour le Canvas natif
    pub close_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub step_tab_hitboxes: Vec<(usize, i32, i32, u32, u32)>,
    pub archetype_hitboxes: Vec<(&'static str, i32, i32, u32, u32)>,
    pub genre_card_hitboxes: Vec<(GameGenre, i32, i32, u32, u32)>,
    pub dimension_hitboxes: Vec<(GameDimension, i32, i32, u32, u32)>,
    pub camera_hitboxes: Vec<(CameraType, i32, i32, u32, u32)>,
    pub movement_hitboxes: Vec<(MovementDirections, i32, i32, u32, u32)>,
    pub ability_jump_hitbox: Option<(i32, i32, u32, u32)>,
    pub ability_dash_hitbox: Option<(i32, i32, u32, u32)>,
    pub ability_shoot_hitbox: Option<(i32, i32, u32, u32)>,
    pub ability_melee_hitbox: Option<(i32, i32, u32, u32)>,
    pub ability_interact_hitbox: Option<(i32, i32, u32, u32)>,
    pub world_gravity_hitbox: Option<(i32, i32, u32, u32)>,
    pub elem_collectible_hitbox: Option<(i32, i32, u32, u32)>,
    pub elem_door_hitbox: Option<(i32, i32, u32, u32)>,
    pub elem_chest_hitbox: Option<(i32, i32, u32, u32)>,
    pub elem_lever_hitbox: Option<(i32, i32, u32, u32)>,
    pub elem_trap_hitbox: Option<(i32, i32, u32, u32)>,
    pub feat_inventory_hitbox: Option<(i32, i32, u32, u32)>,
    pub feat_dialogues_hitbox: Option<(i32, i32, u32, u32)>,
    pub feat_quests_hitbox: Option<(i32, i32, u32, u32)>,
    pub feat_boss_hitbox: Option<(i32, i32, u32, u32)>,
    pub feat_score_hitbox: Option<(i32, i32, u32, u32)>,
    pub feat_waves_hitbox: Option<(i32, i32, u32, u32)>,
    pub perf_hitboxes: Vec<(PerformanceProfile, i32, i32, u32, u32)>,
    pub prev_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub next_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub apply_btn_hitbox: Option<(i32, i32, u32, u32)>,
    pub export_recipe_btn_hitbox: Option<(i32, i32, u32, u32)>,
}

impl GameRecipeWizardState {
    pub fn new() -> Self {
        let mut recipe = KaractiGameRecipe::new("Mon Aventure Épique", "Créateur KaractiRage", GameGenre::Adventure);
        let rep = DependencyResolver::resolve(&mut recipe);
        Self {
            is_open: false,
            step: 1,
            recipe,
            validation_report: Some(rep),
            selected_archetype: None,
            close_btn_hitbox: None,
            step_tab_hitboxes: Vec::new(),
            archetype_hitboxes: Vec::new(),
            genre_card_hitboxes: Vec::new(),
            dimension_hitboxes: Vec::new(),
            camera_hitboxes: Vec::new(),
            movement_hitboxes: Vec::new(),
            ability_jump_hitbox: None,
            ability_dash_hitbox: None,
            ability_shoot_hitbox: None,
            ability_melee_hitbox: None,
            ability_interact_hitbox: None,
            world_gravity_hitbox: None,
            elem_collectible_hitbox: None,
            elem_door_hitbox: None,
            elem_chest_hitbox: None,
            elem_lever_hitbox: None,
            elem_trap_hitbox: None,
            feat_inventory_hitbox: None,
            feat_dialogues_hitbox: None,
            feat_quests_hitbox: None,
            feat_boss_hitbox: None,
            feat_score_hitbox: None,
            feat_waves_hitbox: None,
            perf_hitboxes: Vec::new(),
            prev_btn_hitbox: None,
            next_btn_hitbox: None,
            apply_btn_hitbox: None,
            export_recipe_btn_hitbox: None,
        }
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.step = 1;
        self.run_audit();
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn next_step(&mut self) {
        if self.step < 6 {
            self.step += 1;
            self.run_audit();
        }
    }

    pub fn prev_step(&mut self) {
        if self.step > 1 {
            self.step -= 1;
        }
    }

    pub fn run_audit(&mut self) {
        self.validation_report = Some(DependencyResolver::resolve(&mut self.recipe));
    }

    pub fn apply_archetype(&mut self, key: &'static str) {
        self.selected_archetype = Some(key);
        match key {
            "dungeon" => {
                self.recipe = KaractiGameRecipe::new("Donjon & Énigmes", "Atelier Studio", GameGenre::Adventure);
                self.recipe.player.directions = MovementDirections::FourDirections;
                self.recipe.player.combat = CombatStyle::Melee;
                self.recipe.player.abilities.interact = true;
                self.recipe.world.gravity_enabled = false;
                self.recipe.world.interactive_elements = vec![
                    InteractiveElement::Collectible,
                    InteractiveElement::Door,
                    InteractiveElement::Chest,
                    InteractiveElement::Lever,
                ];
                self.recipe.gameplay.inventory = true;
                self.recipe.gameplay.dialogues = true;
                self.recipe.gameplay.quest_objectives = true;
            }
            "platformer" => {
                self.recipe = KaractiGameRecipe::new("Cyber Platformer 2D", "Atelier Studio", GameGenre::Platformer);
                self.recipe.dimension = GameDimension::TwoD;
                self.recipe.player.directions = MovementDirections::HorizontalOnly;
                self.recipe.player.abilities.jump = true;
                self.recipe.player.abilities.dash = true;
                self.recipe.world.gravity_enabled = true;
                self.recipe.world.interactive_elements = vec![
                    InteractiveElement::Collectible,
                    InteractiveElement::Door,
                    InteractiveElement::MovingPlatform,
                    InteractiveElement::Trap,
                ];
                self.recipe.gameplay.inventory = false;
                self.recipe.gameplay.dialogues = false;
                self.recipe.gameplay.quest_objectives = true;
            }
            "shootemup" => {
                self.recipe = KaractiGameRecipe::new("Shmup Stellaire", "Atelier Studio", GameGenre::ShootEmUp);
                self.recipe.player.directions = MovementDirections::FreeInertia;
                self.recipe.player.combat = CombatStyle::Projectiles;
                self.recipe.player.abilities.dash = true;
                self.recipe.world.gravity_enabled = false;
                self.recipe.world.interactive_elements = vec![InteractiveElement::Collectible];
                self.recipe.gameplay.wave_spawner = true;
                self.recipe.gameplay.score_system = true;
            }
            "sokoban" => {
                self.recipe = KaractiGameRecipe::new("Puzzle Sokoban", "Atelier Studio", GameGenre::Puzzle);
                self.recipe.player.directions = MovementDirections::Grid;
                self.recipe.camera = CameraType::Fixed;
                self.recipe.world.gravity_enabled = false;
                self.recipe.world.interactive_elements = vec![
                    InteractiveElement::Door,
                    InteractiveElement::Lever,
                    InteractiveElement::Collectible,
                ];
                self.recipe.gameplay.inventory = false;
                self.recipe.gameplay.quest_objectives = true;
            }
            "boss" => {
                self.recipe = KaractiGameRecipe::new("Arène du Titan", "Atelier Studio", GameGenre::Combat);
                self.recipe.player.directions = MovementDirections::EightDirections;
                self.recipe.player.combat = CombatStyle::Melee;
                self.recipe.player.abilities.dash = true;
                self.recipe.player.max_health = 8;
                self.recipe.world.gravity_enabled = false;
                self.recipe.world.interactive_elements = vec![InteractiveElement::Door];
                self.recipe.gameplay.boss_fight = true;
                self.recipe.gameplay.inventory = true;
            }
            _ => {}
        }
        self.step = 6;
        self.run_audit();
    }

    pub fn clicked_step_tab(&self, mx: f32, my: f32) -> Option<usize> {
        for &(step, x, y, w, h) in &self.step_tab_hitboxes {
            if mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32 {
                return Some(step);
            }
        }
        None
    }

    pub fn clicked_archetype(&self, mx: f32, my: f32) -> Option<&'static str> {
        for &(key, x, y, w, h) in &self.archetype_hitboxes {
            if mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32 {
                return Some(key);
            }
        }
        None
    }

    pub fn clicked_genre(&self, mx: f32, my: f32) -> Option<GameGenre> {
        for &(genre, x, y, w, h) in &self.genre_card_hitboxes {
            if mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32 {
                return Some(genre);
            }
        }
        None
    }

    pub fn clicked_dimension(&self, mx: f32, my: f32) -> Option<GameDimension> {
        for &(dim, x, y, w, h) in &self.dimension_hitboxes {
            if mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32 {
                return Some(dim);
            }
        }
        None
    }

    pub fn clicked_camera(&self, mx: f32, my: f32) -> Option<CameraType> {
        for &(cam, x, y, w, h) in &self.camera_hitboxes {
            if mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32 {
                return Some(cam);
            }
        }
        None
    }

    pub fn clicked_movement(&self, mx: f32, my: f32) -> Option<MovementDirections> {
        for &(mvt, x, y, w, h) in &self.movement_hitboxes {
            if mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32 {
                return Some(mvt);
            }
        }
        None
    }

    pub fn clicked_perf(&self, mx: f32, my: f32) -> Option<PerformanceProfile> {
        for &(perf, x, y, w, h) in &self.perf_hitboxes {
            if mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32 {
                return Some(perf);
            }
        }
        None
    }

    pub fn is_hitbox_clicked(hitbox: Option<(i32, i32, u32, u32)>, mx: f32, my: f32) -> bool {
        hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            mode: EditorMode::Play,
            selected: SelectedEntity::None,
            active_tool: StudioTool::Select,
            brush_tile: BrushTile::Wall,
            last_painted_cell: None,
            is_detailed_view: false, // Vue simplifiée par défaut (Ergonomie KaractiRage)
            is_dragging: false,
            drag_offset_x: 0.0,
            drag_offset_y: 0.0,
            mouse_x: 0.0,
            mouse_y: 0.0,
            active_wire: None,
            pin_hitboxes: Vec::new(),
            connection_hitboxes: Vec::new(),
            show_node_palette: false,
            palette_hitboxes: Vec::new(),
            save_btn_hitbox: None,
            top_save_btn_hitbox: None,
            add_node_btn_hitbox: None,
            view_btn_hitbox: None,
            show_spawn_palette: false,
            spawn_palette_hitboxes: Vec::new(),
            spawn_btn_hitbox: None,
            duplicate_btn_hitbox: None,
            delete_btn_hitbox: None,
            tool_select_btn_hitbox: None,
            tool_brush_btn_hitbox: None,
            tool_eraser_btn_hitbox: None,
            tool_tile_cycle_btn_hitbox: None,
            add_comp_hitboxes: Vec::new(),
            show_help_modal: false,
            help_btn_hitbox: None,
            mode_toggle_btn_hitbox: None,
            genre_btn_hitbox: None,
            radar_btn_hitbox: None,
            grid_btn_hitbox: None,
            telemetry_btn_hitbox: None,
            audio_btn_hitbox: None,
            reset_btn_hitbox: None,
            show_console_drawer: false,
            console_toggle_btn_hitbox: None,
            play_tab_hitbox: None,
            studio_tab_hitbox: None,
            wizard: GuidedWizardState::new(),
            guided_btn_hitbox: None,
            undo_stack: UndoStack::default(),
            undo_btn_hitbox: None,
            redo_btn_hitbox: None,
            active_tooltip: None,
            show_welcome_modal: false,
            welcome_tab: 0,
            welcome_tab_hitboxes: Vec::new(),
            welcome_scene_hitboxes: Vec::new(),
            welcome_archetype_hitboxes: Vec::new(),
            welcome_play_btn_hitbox: None,
            welcome_studio_btn_hitbox: None,
            welcome_guide_btn_hitbox: None,
            welcome_recipe_btn_hitbox: None,
            welcome_close_btn_hitbox: None,
            hub_btn_hitbox: None,
            guide_tab: 0,
            guide_tab_btn_hitboxes: Vec::new(),
            guide_step_done: [false; 6],
            guide_close_btn_hitbox: None,
            show_intent_bar: false,
            intent_input_text: String::new(),
            intent_bar_btn_hitbox: None,
            intent_preset_hitboxes: Vec::new(),
            intent_close_btn_hitbox: None,
            pending_ai_patch: None,
            ai_patch_accept_hitbox: None,
            ai_patch_modify_hitbox: None,
            ai_patch_reject_hitbox: None,
            ai_patch_history: Vec::new(),
            intent_submit_btn_hitbox: None,
            ai_config: crate::ai_client::AiClientConfig::load_or_default(),
            ai_provider_btn_hitboxes: Vec::new(),
            ai_generation_in_progress: false,
            ai_status_text: String::new(),
            ai_test_btn_hitbox: None,
            ai_test_result: None,
            show_export_modal: false,
            export_title: "Mon Jeu KaractiRage".to_string(),
            export_author: "Créateur Indépendant".to_string(),
            export_format_idx: 0,
            export_btn_hitbox: None,
            export_action_btn_hitbox: None,
            export_format_hitboxes: Vec::new(),
            export_close_btn_hitbox: None,
            recipe_wizard: GameRecipeWizardState::new(),
            recipe_btn_hitbox: None,
        }
    }

    pub fn is_ai_test_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((x, y, w, h)) = self.ai_test_btn_hitbox {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        } else {
            false
        }
    }

    pub fn clicked_ai_provider(&self, mx: f32, my: f32) -> Option<crate::ai_client::AiProvider> {
        for &(provider, x, y, w, h) in &self.ai_provider_btn_hitboxes {
            if mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32 {
                return Some(provider);
            }
        }
        None
    }

    pub fn clicked_welcome_tab(&self, mx: f32, my: f32) -> Option<usize> {
        for &(tab_idx, x, y, w, h) in &self.welcome_tab_hitboxes {
            if mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32 {
                return Some(tab_idx);
            }
        }
        None
    }

    pub fn clicked_welcome_scene(&self, mx: f32, my: f32) -> Option<(usize, bool)> {
        for &(idx, is_studio, x, y, w, h) in &self.welcome_scene_hitboxes {
            if mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32 {
                return Some((idx, is_studio));
            }
        }
        None
    }

    pub fn clicked_welcome_archetype(&self, mx: f32, my: f32) -> Option<&'static str> {
        for &(arch_key, x, y, w, h) in &self.welcome_archetype_hitboxes {
            if mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32 {
                return Some(arch_key);
            }
        }
        None
    }

    pub fn is_hub_btn_clicked(&self, mx: f32, my: f32) -> bool {
        self.hub_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn clicked_guide_tab(&self, mx: f32, my: f32) -> Option<usize> {
        for &(tab_idx, x, y, w, h) in &self.guide_tab_btn_hitboxes {
            if mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32 {
                return Some(tab_idx);
            }
        }
        None
    }

    pub fn is_welcome_play_clicked(&self, mx: f32, my: f32) -> bool {
        self.welcome_play_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_welcome_studio_clicked(&self, mx: f32, my: f32) -> bool {
        self.welcome_studio_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_welcome_guide_clicked(&self, mx: f32, my: f32) -> bool {
        self.welcome_guide_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_welcome_close_clicked(&self, mx: f32, my: f32) -> bool {
        self.welcome_close_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_welcome_recipe_clicked(&self, mx: f32, my: f32) -> bool {
        self.welcome_recipe_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_recipe_btn_clicked(&self, mx: f32, my: f32) -> bool {
        self.recipe_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn clicked_intent_preset(&self, mx: f32, my: f32) -> Option<usize> {
        for &(idx, x, y, w, h) in &self.intent_preset_hitboxes {
            if mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32 {
                return Some(idx);
            }
        }
        None
    }

    pub fn is_intent_close_clicked(&self, mx: f32, my: f32) -> bool {
        self.intent_close_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_intent_bar_btn_clicked(&self, mx: f32, my: f32) -> bool {
        self.intent_bar_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_intent_submit_clicked(&self, mx: f32, my: f32) -> bool {
        self.intent_submit_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_ai_patch_accept_clicked(&self, mx: f32, my: f32) -> bool {
        self.ai_patch_accept_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_ai_patch_modify_clicked(&self, mx: f32, my: f32) -> bool {
        self.ai_patch_modify_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_ai_patch_reject_clicked(&self, mx: f32, my: f32) -> bool {
        self.ai_patch_reject_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_mode_toggle_btn_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((x, y, w, h)) = self.mode_toggle_btn_hitbox {
            mx >= x as f32 && mx <= (x as f32 + w as f32) && my >= y as f32 && my <= (y as f32 + h as f32)
        } else {
            false
        }
    }

    pub fn is_play_tab_clicked(&self, mx: f32, my: f32) -> bool {
        self.play_tab_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_studio_tab_clicked(&self, mx: f32, my: f32) -> bool {
        self.studio_tab_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_console_toggle_clicked(&self, mx: f32, my: f32) -> bool {
        self.console_toggle_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_genre_btn_clicked(&self, mx: f32, my: f32) -> bool {
        self.genre_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_radar_btn_clicked(&self, mx: f32, my: f32) -> bool {
        self.radar_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_grid_btn_clicked(&self, mx: f32, my: f32) -> bool {
        self.grid_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_telemetry_btn_clicked(&self, mx: f32, my: f32) -> bool {
        self.telemetry_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_audio_btn_clicked(&self, mx: f32, my: f32) -> bool {
        self.audio_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn is_reset_btn_clicked(&self, mx: f32, my: f32) -> bool {
        self.reset_btn_hitbox.map_or(false, |(x, y, w, h)| {
            mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32
        })
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            EditorMode::Play => EditorMode::Edit,
            EditorMode::Edit => {
                self.selected = SelectedEntity::None;
                self.is_dragging = false;
                self.active_wire = None;
                self.show_node_palette = false;
                self.show_spawn_palette = false;
                self.show_help_modal = false;
                self.active_tool = StudioTool::Select;
                self.last_painted_cell = None;
                EditorMode::Play
            }
        };
    }

    pub fn set_tool(&mut self, tool: StudioTool) {
        self.active_tool = tool;
        self.last_painted_cell = None;
        if tool != StudioTool::Select {
            self.selected = SelectedEntity::None;
            self.is_dragging = false;
            self.active_wire = None;
            self.show_node_palette = false;
            self.show_spawn_palette = false;
        }
    }

    pub fn cycle_brush_tile(&mut self) -> BrushTile {
        self.brush_tile = self.brush_tile.next();
        self.brush_tile
    }

    pub fn toggle_help(&mut self) {
        self.show_help_modal = !self.show_help_modal;
    }

    pub fn toggle_view_detail(&mut self) {
        self.is_detailed_view = !self.is_detailed_view;
        self.active_wire = None;
        self.show_node_palette = false;
        self.show_spawn_palette = false;
    }

    pub fn toggle_palette(&mut self) {
        self.show_node_palette = !self.show_node_palette;
        if self.show_node_palette {
            self.show_spawn_palette = false;
        }
    }

    pub fn toggle_spawn_palette(&mut self) {
        self.show_spawn_palette = !self.show_spawn_palette;
        if self.show_spawn_palette {
            self.show_node_palette = false;
        }
    }

    pub fn start_wire(&mut self, node_id: String, pin_id: String, start_x: i32, start_y: i32) {
        self.active_wire = Some(ActiveWire {
            from_node: node_id,
            from_pin: pin_id,
            start_x,
            start_y,
        });
    }

    pub fn cancel_wire(&mut self) {
        self.active_wire = None;
    }

    pub fn find_pin_at(&self, mx: f32, my: f32) -> Option<&PinHitbox> {
        self.pin_hitboxes.iter().find(|hit| {
            mx >= hit.x as f32 && mx <= (hit.x + hit.w as i32) as f32
                && my >= hit.y as f32 && my <= (hit.y + hit.h as i32) as f32
        })
    }

    pub fn find_connection_cut_at(&self, mx: f32, my: f32) -> Option<&ConnectionHitbox> {
        self.connection_hitboxes.iter().find(|hit| {
            mx >= hit.x as f32 && mx <= (hit.x + hit.w as i32) as f32
                && my >= hit.y as f32 && my <= (hit.y + hit.h as i32) as f32
        })
    }

    pub fn find_palette_item_at(&self, mx: f32, my: f32) -> Option<&PaletteItemHitbox> {
        self.palette_hitboxes.iter().find(|hit| {
            mx >= hit.x as f32 && mx <= (hit.x + hit.w as i32) as f32
                && my >= hit.y as f32 && my <= (hit.y + hit.h as i32) as f32
        })
    }

    pub fn find_spawn_palette_item_at(&self, mx: f32, my: f32) -> Option<&PaletteItemHitbox> {
        self.spawn_palette_hitboxes.iter().find(|hit| {
            mx >= hit.x as f32 && mx <= (hit.x + hit.w as i32) as f32
                && my >= hit.y as f32 && my <= (hit.y + hit.h as i32) as f32
        })
    }

    pub fn find_add_comp_at(&self, mx: f32, my: f32) -> Option<&str> {
        self.add_comp_hitboxes.iter().find(|(_, bx, by, bw, bh)| {
            mx >= *bx as f32 && mx <= (*bx + *bw as i32) as f32
                && my >= *by as f32 && my <= (*by + *bh as i32) as f32
        }).map(|(comp, _, _, _, _)| comp.as_str())
    }

    pub fn is_save_btn_clicked(&self, mx: f32, my: f32) -> bool {
        let in_box = |opt: Option<(i32, i32, u32, u32)>| {
            if let Some((bx, by, bw, bh)) = opt {
                mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
            } else {
                false
            }
        };
        in_box(self.save_btn_hitbox) || in_box(self.top_save_btn_hitbox)
    }

    pub fn is_add_node_btn_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.add_node_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_view_btn_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.view_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_spawn_btn_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.spawn_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_duplicate_btn_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.duplicate_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_delete_btn_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.delete_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_help_btn_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.help_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_tool_select_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.tool_select_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_tool_brush_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.tool_brush_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_tool_eraser_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.tool_eraser_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_tool_tile_cycle_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.tool_tile_cycle_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_guided_btn_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.guided_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_undo_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.undo_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_redo_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.redo_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_export_btn_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.export_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_export_action_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.export_action_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn is_export_close_clicked(&self, mx: f32, my: f32) -> bool {
        if let Some((bx, by, bw, bh)) = self.export_close_btn_hitbox {
            mx >= bx as f32 && mx <= (bx + bw as i32) as f32 && my >= by as f32 && my <= (by + bh as i32) as f32
        } else {
            false
        }
    }

    pub fn clicked_export_format(&self, mx: f32, my: f32) -> Option<usize> {
        for &(fmt_idx, x, y, w, h) in &self.export_format_hitboxes {
            if mx >= x as f32 && mx <= (x + w as i32) as f32 && my >= y as f32 && my <= (y + h as i32) as f32 {
                return Some(fmt_idx);
            }
        }
        None
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}
