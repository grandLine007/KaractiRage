//! Import d'une carte Tiled (`.tmx`) vers une scène KaractiRage (`.scene.json`).
//!
//! Usage :
//! ```bash
//! cargo run -p engine-core --bin tiled_scene -- <carte.tmx> <sortie.scene.json>
//! ```
//!
//! Codes de sortie : `0` succès, `1` erreur d'import ou d'écriture,
//! `2` usage invalide.

use std::collections::BTreeMap;
use std::path::Path;

use karactirage_engine::tiled_import::import_tmx;

const USAGE: &str = "Usage : tiled_scene <carte.tmx> <sortie.scene.json>";

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();

    if arguments.len() == 1 && matches!(arguments[0].as_str(), "-h" | "--help") {
        println!("{USAGE}");
        return;
    }
    if arguments.len() != 2 {
        eprintln!("{USAGE}");
        std::process::exit(2);
    }

    let carte = Path::new(&arguments[0]);
    let sortie = Path::new(&arguments[1]);

    let rapport = match import_tmx(carte) {
        Ok(rapport) => rapport,
        Err(erreur) => {
            eprintln!("Erreur d'import Tiled : {erreur}");
            std::process::exit(1);
        }
    };

    let json = match serde_json::to_string_pretty(&rapport.scene) {
        Ok(json) => json,
        Err(erreur) => {
            eprintln!("Erreur de sérialisation de la scène : {erreur}");
            std::process::exit(1);
        }
    };

    if let Some(parent) = sortie.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(erreur) = std::fs::create_dir_all(parent) {
                eprintln!(
                    "Impossible de créer le répertoire « {} » : {erreur}",
                    parent.display()
                );
                std::process::exit(1);
            }
        }
    }
    if let Err(erreur) = std::fs::write(sortie, &json) {
        eprintln!(
            "Impossible d'écrire la scène « {} » : {erreur}",
            sortie.display()
        );
        std::process::exit(1);
    }

    let mut par_tag: BTreeMap<&str, usize> = BTreeMap::new();
    for entite in &rapport.scene.entities {
        *par_tag.entry(entite.tag.as_str()).or_insert(0) += 1;
    }
    let repartition = par_tag
        .iter()
        .map(|(tag, nombre)| format!("{tag}={nombre}"))
        .collect::<Vec<_>>()
        .join(", ");

    println!("Carte Tiled importée : {}", carte.display());
    println!(
        "Scène écrite : {} ({} octets)",
        sortie.display(),
        json.len()
    );
    println!(
        "Dimensions de la carte : {} × {} px",
        rapport.map_size.0, rapport.map_size.1
    );
    println!("Entités importées : {}", rapport.entities_imported);
    println!("Répartition : {repartition}");
    println!(
        "Couches de tuiles aplaties : {}",
        rapport.tile_layers_flattened
    );
    if rapport.warnings.is_empty() {
        println!("Avertissements : aucun");
    } else {
        println!("Avertissements ({}) :", rapport.warnings.len());
        for avertissement in &rapport.warnings {
            println!("  - {avertissement}");
        }
    }
}
