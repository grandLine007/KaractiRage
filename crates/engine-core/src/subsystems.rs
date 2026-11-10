//! Sous-systèmes data-driven exposés à l'IA : UI, sauvegarde, export, 3D (caméra/lumières).
//!
//! Ces modèles rendent « générables » des artefacts exigés par le plan L3 (UI/menus, save,
//! export) et L2 3D (caméra/lumières). Ils sont purement **déclaratifs** : aucun code généré.

use serde::{Deserialize, Serialize};

/// Type d'écran UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UiScreenKind {
    Hud,
    Menu,
    Pause,
    GameOver,
    Victory,
}

/// Élément d'un écran UI (texte, barre, bouton, icône).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UiElement {
    pub id: String,
    /// "text", "bar", "button", "icon".
    pub kind: String,
    #[serde(default)]
    pub label: Option<String>,
    /// Variable liée ("player_hp", "score", "objective").
    #[serde(default)]
    pub bind: Option<String>,
}

/// Écran UI déclaratif (HUD, menu, pause, game over, victoire).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UiScreen {
    pub id: String,
    pub kind: UiScreenKind,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub elements: Vec<UiElement>,
}

/// Configuration de sauvegarde / progression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SaveConfig {
    pub enabled: bool,
    pub slots: u32,
    pub auto_save: bool,
}

impl Default for SaveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            slots: 3,
            auto_save: true,
        }
    }
}

/// Cible d'export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExportTargetKind {
    Html,
    Package,
    Desktop,
    Wasm,
}

/// Caméra 3D (position + cible).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Camera3DData {
    pub position: [f32; 3],
    pub target: [f32; 3],
}

/// Lumière 3D (type, position, couleur, intensité).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Light3DData {
    /// "point", "directional", "ambient".
    pub kind: String,
    pub position: [f32; 3],
    pub color: u32,
    pub intensity: f32,
}
