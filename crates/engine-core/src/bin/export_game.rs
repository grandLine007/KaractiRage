//! Export d'un projet `.karacti` en HTML5, avec pré-vol bloquant.
//!
//! Usage :
//! ```bash
//! cargo run -p engine-core --bin export_game -- assets/projects/jeu_reference.karacti /tmp/jeu.html
//! ```
//!
//! Sortie 0 = fichier écrit ; 1 = export refusé par le pré-vol (la liste est affichée) ;
//! 2 = argument invalide. Utilisé par `apps/studio/scripts/export-smoke.mjs`.

use std::process::ExitCode;

use karactirage_engine::exporter::StandaloneExporter;
use karactirage_engine::game::GameState;
use karactirage_engine::project::KaractiProject;

/// Scène de base servant à construire un `GameState` avant `load_project`.
fn base_scene() -> &'static str {
    if std::path::Path::new("assets/scenes/level_01.scene.json").exists() {
        "assets/scenes/level_01.scene.json"
    } else {
        "../../assets/scenes/level_01.scene.json"
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(project_path), Some(out_path)) = (args.next(), args.next()) else {
        eprintln!("usage : export_game <projet.karacti> <sortie.html>");
        return ExitCode::from(2);
    };

    let project = match KaractiProject::load_from_file(&project_path) {
        Ok(project) => project,
        Err(error) => {
            eprintln!("projet illisible ({project_path}) : {error}");
            return ExitCode::from(2);
        }
    };
    let title = project.metadata.title.clone();
    let author = project.metadata.author.clone();

    let mut game = match GameState::load_from_scene(base_scene()) {
        Ok(game) => game,
        Err(error) => {
            eprintln!("scène de base illisible : {error}");
            return ExitCode::from(2);
        }
    };
    game.load_project(project);

    let report = StandaloneExporter::preflight(&game);
    for issue in &report.issues {
        eprintln!("[{}] {} — {}", if issue.severity == karactirage_engine::exporter::PreflightSeverity::Blocking { "blocage" } else { "avertissement" }, issue.code, issue.message);
    }
    if !report.ok {
        eprintln!("export refusé : {}", report.summary());
        return ExitCode::from(1);
    }

    let html = match StandaloneExporter::export_html_checked(&game, &title, &author) {
        Ok(html) => html,
        Err(report) => {
            eprintln!("export refusé : {}", report.summary());
            return ExitCode::from(1);
        }
    };

    if let Some(parent) = std::path::Path::new(&out_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(error) = std::fs::write(&out_path, &html) {
        eprintln!("écriture impossible ({out_path}) : {error}");
        return ExitCode::from(2);
    }

    println!(
        "export OK — {} entités, {} graphes KVL, {} Ko, empreinte {}",
        game.entities.len(),
        game.kvl_graphs.len(),
        html.len() / 1024,
        StandaloneExporter::export_fingerprint(&html)
    );
    ExitCode::SUCCESS
}
