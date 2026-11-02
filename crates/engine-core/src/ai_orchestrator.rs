//! Orchestrateur : relie intention → prompt assemblé → LLM → patch (boucle fermée).

use crate::ai_client::{AiClient, AiClientConfig, LlmBackend};
use crate::ai_history::{new_run_id, record_run, AiRunLog, AiRunStep};
use crate::ai_intent::{normalize_intent, IntentEnvelope};
use crate::ai_plan::{plan_for, AiPlan};
use crate::ai_patch::{AiPatch, AiPatchBuilder};
use crate::ai_prompt::PromptAssembler;
use std::path::Path;

/// Génère un plan (déterministe, sans LLM) à partir d'un texte libre.
pub fn generate_plan_from_intent(raw_text: &str) -> (IntentEnvelope, AiPlan) {
    let envelope = normalize_intent(raw_text);
    let plan = plan_for(&envelope);
    (envelope, plan)
}

/// Génère un patch IA (niveau) à partir d'une intention, via le prompt assemblé et le LLM.
pub fn generate_patch_from_intent(
    config: &AiClientConfig,
    intent: &str,
    backend: &dyn LlmBackend,
) -> Result<AiPatch, String> {
    let assembled = PromptAssembler::assemble_full(config.provider, intent, None, None);
    let payload = AiClient::send_prompt_with_system_prompt(config, &assembled.text, intent, backend)?;
    let patch = AiPatchBuilder::build_from_generated_payload(&payload, intent, (0.0, 0.0), 0, &[], None);
    Ok(patch)
}

/// Génère un patch et enregistre un journal de run (traçabilité §9).
pub fn generate_and_record_patch(
    config: &AiClientConfig,
    intent: &str,
    backend: &dyn LlmBackend,
    root: &Path,
) -> Result<AiPatch, String> {
    let patch = generate_patch_from_intent(config, intent, backend)?;
    let envelope = normalize_intent(intent);
    let plan = plan_for(&envelope);
    let assembled = PromptAssembler::assemble_full(config.provider, intent, None, None);

    let log = AiRunLog {
        run_id: new_run_id(&config.provider.label(), intent),
        dictionary_version: crate::ai_dictionary::DICTIONARY_VERSION.to_string(),
        provider: config.provider.label().to_string(),
        model: config
            .custom_model
            .clone()
            .unwrap_or_else(|| config.provider.default_model().to_string()),
        prompt_hash: assembled.hash,
        intent_id: envelope.intent_id,
        steps: plan
            .steps
            .iter()
            .map(|s| AiRunStep {
                step: s.id.clone(),
                artifact: s.artifact.clone(),
                validation: if patch.sandbox_report.static_check_passed {
                    "passed".into()
                } else {
                    "failed".into()
                },
                attempts: 1,
            })
            .collect(),
        applied: false,
        error: None,
    };
    let _ = record_run(root, &log);
    Ok(patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_client::AiProvider;
    use crate::ai_intent::IntentTarget;

    struct MockBackend {
        response: String,
    }

    impl LlmBackend for MockBackend {
        fn post_json(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            _body: &str,
            _timeout_secs: u64,
        ) -> Result<String, String> {
            Ok(self.response.clone())
        }
    }

    #[test]
    fn generate_plan_from_intent_is_valid_and_deterministic() {
        let (envelope, plan) = generate_plan_from_intent("Crée un jeu complet de plateforme 2D");
        assert_eq!(envelope.target, IntentTarget::FullGame);
        assert!(plan.validate().is_ok());
        assert_eq!(plan.steps.len(), 7);
    }

    #[test]
    fn generate_patch_from_intent_wires_assembler_llm_and_builder() {
        let response = r#"{"choices":[{"message":{"content":"{\"title\":\"Mock Level\",\"description\":\"test\",\"entities\":[],\"kvl_actions\":[]}"}}]}"#;
        let backend = MockBackend { response: response.to_string() };
        let config = AiClientConfig {
            provider: AiProvider::LmStudio,
            custom_endpoint: None,
            custom_model: None,
            custom_api_key: None,
            timeout_seconds: 10,
        };

        let patch = generate_patch_from_intent(&config, "Crée un niveau plateforme", &backend)
            .expect("patch généré");
        assert_eq!(patch.title, "Mock Level");
        assert!(patch.sandbox_report.static_check_passed);
    }

    #[test]
    fn generate_and_record_patch_writes_run_log() {
        let response = r#"{"choices":[{"message":{"content":"{\"title\":\"Mock Level\",\"description\":\"test\",\"entities\":[],\"kvl_actions\":[]}"}}]}"#;
        let backend = MockBackend { response: response.to_string() };
        let config = AiClientConfig {
            provider: AiProvider::LmStudio,
            custom_endpoint: None,
            custom_model: None,
            custom_api_key: None,
            timeout_seconds: 10,
        };
        let root = std::env::temp_dir().join(format!("karacti_ai_run_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);

        let patch = generate_and_record_patch(&config, "Crée un niveau plateforme", &backend, &root)
            .expect("patch généré");
        assert_eq!(patch.title, "Mock Level");

        let runs = crate::ai_history::list_runs(&root);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].provider, "LM Studio (Local)");
        assert!(!runs[0].steps.is_empty());
        assert!(!runs[0].prompt_hash.is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }
}
