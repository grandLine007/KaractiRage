//! Binaire de compilation du dictionnaire IA.
//!
//! Usage :
//! ```bash
//! cargo run -p engine-core --bin ai_dictionary -- --out dist/ai/current
//! ```
//!
//! Options : `--root <dir>` (racine du dépôt, défaut : manifest dir /../..),
//!           `--out <dir>` (sortie, défaut : <root>/dist/ai/current).

use std::env;
use std::path::PathBuf;

use karactirage_engine::ai_dictionary::{AiDictionary, DICTIONARY_VERSION};

fn main() {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => {
                let value = args.next().expect("valeur pour --root");
                root = PathBuf::from(value);
            }
            "--out" => {
                let value = args.next().expect("valeur pour --out");
                out = Some(PathBuf::from(value));
            }
            other => {
                eprintln!("argument inconnu : {other}");
                std::process::exit(2);
            }
        }
    }

    let root = root.canonicalize().unwrap_or(root);
    let out = out.unwrap_or_else(|| root.join("dist").join("ai").join("current"));

    match AiDictionary::build(&root, &out) {
        Ok(manifest) => {
            println!(
                "Dictionnaire IA {} (engine_api {}) écrit dans {} ({} fichiers)",
                DICTIONARY_VERSION,
                manifest.engine_api,
                out.display(),
                manifest.files.len()
            );
        }
        Err(error) => {
            eprintln!("Échec de compilation du dictionnaire : {error}");
            std::process::exit(1);
        }
    }
}
