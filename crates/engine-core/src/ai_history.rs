//! Traçabilité des runs IA : journal JSON par exécution (plan §9).
//!
//! Chaque run écrit `.karacti/ai-runs/<run_id>.json` (version, provider, modèle, hash de
//! prompt, plan, étapes, statut). « Toujours mesurer. »

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ai_dictionary::hash_text;

/// Étape d'un run (op + validation).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AiRunStep {
    pub step: String,
    pub artifact: String,
    pub validation: String,
    pub attempts: u32,
}

/// Journal d'un run IA.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AiRunLog {
    pub run_id: String,
    pub dictionary_version: String,
    pub provider: String,
    pub model: String,
    pub prompt_hash: String,
    pub intent_id: String,
    pub steps: Vec<AiRunStep>,
    pub applied: bool,
    pub error: Option<String>,
}

/// Identifiant de run déterministe (provider + intention).
pub fn new_run_id(provider: &str, intent: &str) -> String {
    let hash = hash_text(format!("{provider}:{intent}").as_bytes());
    format!("run_{}", &hash[..12])
}

/// Enregistre un run dans `<root>/.karacti/ai-runs/<run_id>.json`.
pub fn record_run(root: &Path, log: &AiRunLog) -> Result<PathBuf, String> {
    let dir = root.join(".karacti").join("ai-runs");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", log.run_id));
    let json = serde_json::to_string_pretty(log).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Liste les runs enregistrés (triés par identifiant).
pub fn list_runs(root: &Path) -> Vec<AiRunLog> {
    let dir = root.join(".karacti").join("ai-runs");
    let mut logs = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(log) = serde_json::from_str::<AiRunLog>(&content) {
                        logs.push(log);
                    }
                }
            }
        }
    }
    logs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_list_runs_roundtrip() {
        let root = std::env::temp_dir().join(format!("karacti_ai_history_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let log = AiRunLog {
            run_id: "run_test".into(),
            dictionary_version: "2026.11.1".into(),
            provider: "LM Studio".into(),
            model: "nemotron".into(),
            prompt_hash: "abc123".into(),
            intent_id: "int_x".into(),
            steps: vec![AiRunStep {
                step: "s1".into(),
                artifact: "project".into(),
                validation: "passed".into(),
                attempts: 1,
            }],
            applied: false,
            error: None,
        };

        let path = record_run(&root, &log).expect("record");
        assert!(path.exists());

        let runs = list_runs(&root);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run_id, "run_test");
        assert_eq!(runs[0].steps[0].artifact, "project");

        let _ = fs::remove_dir_all(&root);
    }
}
