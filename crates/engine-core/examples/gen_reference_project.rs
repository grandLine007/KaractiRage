//! Générateur **ponctuel** du projet de référence P1.
//!
//! Usage (une seule fois, artefact committé ensuite) :
//! `cargo run -p engine-core --example gen_reference_project`
//!
//! On passe par les types du moteur pour garantir que le `.karacti` produit est
//! désérialisable : le fichier de sortie est la source de vérité, pas ce binaire.

use karactirage_engine::project::{KaractiProject, PROJECT_SCHEMA_VERSION, ProjectMetadata, ProjectSettings};
use karactirage_engine::recipe::GameGenre;
use karactirage_engine::scene::SceneData;
use karactirage_engine::subsystems::{UiElement, UiScreen, UiScreenKind};
use std::collections::HashMap;

fn text_element(id: &str, label: &str) -> UiElement {
    UiElement {
        id: id.to_string(),
        kind: "text".to_string(),
        label: Some(label.to_string()),
        bind: None,
    }
}

fn screen(id: &str, kind: UiScreenKind, title: &str, lines: &[&str]) -> UiScreen {
    UiScreen {
        id: id.to_string(),
        kind,
        title: Some(title.to_string()),
        elements: lines
            .iter()
            .enumerate()
            .map(|(index, line)| text_element(&format!("{id}_l{index}"), line))
            .collect(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let level_1 = SceneData::load_from_file("assets/scenes/ref_niveau_1.scene.json")?;
    let level_2 = SceneData::load_from_file("assets/scenes/ref_niveau_2.scene.json")?;
    let level_3 = SceneData::load_from_file("assets/scenes/ref_niveau_3.scene.json")?;

    let mut variables = HashMap::new();
    variables.insert("score".to_string(), kvl_core::Value::Int(0));
    variables.insert("menu_active".to_string(), kvl_core::Value::Bool(true));
    variables.insert("game_won".to_string(), kvl_core::Value::Bool(false));

    let metadata = ProjectMetadata {
        id: "proj_reference_p1".to_string(),
        title: "Les Émeraudes du Temple".to_string(),
        author: "KaractiRage".to_string(),
        version: "0.3.0".to_string(),
        created_at: "2026-11-10T00:00:00Z".to_string(),
        description: "Jeu de référence P1 : trois niveaux dont une arène de boss, menu, pause, défaite et victoire.".to_string(),
        genre_primary: GameGenre::Adventure,
        genres_secondary: Vec::new(),
        dimension: Some("2D".to_string()),
    };

    let settings = ProjectSettings {
        default_mode: "play".to_string(),
        gravity: 0.0,
        ..ProjectSettings::default()
    };

    let ui_screens = vec![
        screen(
            "menu",
            UiScreenKind::Menu,
            "LES ÉMERAUDES DU TEMPLE",
            &["ENTRÉE — JOUER", "ÉCHAP — PAUSE EN JEU"],
        ),
        screen(
            "pause",
            UiScreenKind::Pause,
            "PAUSE",
            &["ENTRÉE — REPRENDRE", "ÉCHAP — REPRENDRE"],
        ),
        screen(
            "game_over",
            UiScreenKind::GameOver,
            "DÉFAITE",
            &["ENTRÉE — REVENIR AU MENU"],
        ),
        screen(
            "victory",
            UiScreenKind::Victory,
            "VICTOIRE",
            &["ENTRÉE — REVENIR AU MENU"],
        ),
    ];

    let project = KaractiProject {
        schema_version: PROJECT_SCHEMA_VERSION,
        metadata,
        settings,
        recipe: None,
        scene: level_1.clone(),
        scenes: vec![level_1, level_2, level_3],
        kvl_graphs: HashMap::new(),
        kvl_variables: variables,
        ui_screens,
        save_config: Default::default(),
        export_target: None,
        camera3d: None,
        lights: Vec::new(),
        levels: vec!["niveau_1".to_string(), "niveau_2".to_string(), "niveau_3".to_string()],
    };

    project.save_to_file("assets/projects/jeu_reference.karacti")?;
    println!(
        "Projet écrit : {} niveaux, {} écrans",
        project.scenes.len(),
        project.ui_screens.len()
    );
    Ok(())
}
