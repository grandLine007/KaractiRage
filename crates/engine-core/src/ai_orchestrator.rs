//! Orchestrateur : relie intention → prompt assemblé → LLM → patch (boucle fermée).

use crate::ai_client::{AiClient, AiClientConfig, LlmBackend};
use crate::ai_intent::{normalize_intent, IntentEnvelope};
use crate::ai_plan::{plan_for, AiPlan};
use crate::ai_patch::{AiPatch, AiPatchBuilder};
use crate::ai_prompt::PromptAssembler;

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
        assert_eq!(plan.steps.len(), 6);
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
}
