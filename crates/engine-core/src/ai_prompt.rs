//! Assembleur de prompts IA : profils de fournisseur, budgets, retrieval lexical.
//!
//! Respecte le plan §4.6 : le budget est calculé avant l'appel ; en cas de dépassement on
//! retire d'abord les exemples, puis le contexte, puis le sous-ensemble de nœuds — jamais
//! la constitution, le schéma de l'étape ni l'intention.

use kvl_core::NodeSchema;

use crate::ai_client::AiProvider;
use crate::ai_dictionary::{hash_text, AiDictionary, DICTIONARY_VERSION};

/// Constitution embarquée (invariants immuables).
pub const CONSTITUTION: &str = "Règles immuables : sortie JSON brute conforme au schéma de l'étape, sans markdown. Un appel = un artefact. Aucune action/événement inconnu du catalogue. Pas de code généré. Le LLM propose, le moteur dispose.";

/// Rappel du schéma de niveau (l'étape la plus fréquente).
pub const SCHEMA_HINT: &str = "Schéma de niveau (AiGeneratedGamePayload) : title, description, primary_genre, terrain (platforms/hazards), gameplay_loop (objective/win_condition/lose_condition), entities, kvl_actions (event/action/condition/on_true_action/on_false_action/second_action), variables.";

/// Profil de fournisseur : budget de contexte (tokens).
#[derive(Debug, Clone, Copy)]
pub struct ProviderProfile {
    pub provider: AiProvider,
    pub budget_tokens: usize,
}

impl ProviderProfile {
    pub fn for_provider(provider: AiProvider) -> Self {
        let budget_tokens = match provider {
            AiProvider::Ollama | AiProvider::LmStudio => 8_000,
            AiProvider::OfflineLocal => 0,
            _ => 32_000,
        };
        Self {
            provider,
            budget_tokens,
        }
    }
}

/// Bloc de prompt avec son ordre de retrait (0 = jamais retiré, croissant = plus jetable).
#[derive(Debug, Clone)]
pub struct PromptBlock {
    pub label: &'static str,
    pub text: String,
    /// 0 = intouchable ; 1 = gardé en priorité ; plus élevé = retiré en premier.
    pub drop_order: u8,
}

/// Prompt assemblé, prêt à l'envoi, avec sa traçabilité.
#[derive(Debug, Clone)]
pub struct AssembledPrompt {
    pub text: String,
    pub hash: String,
    pub estimated_tokens: usize,
    pub dictionary_version: &'static str,
    /// Libellés des blocs effectivement inclus (diagnostic).
    pub included_blocks: Vec<&'static str>,
}

/// Estimation naïve du nombre de tokens (mots séparés par espaces).
///
/// Suffisante pour le budget local ≤ 8k ; à remplacer par un tokeniseur exact au besoin.
pub fn estimate_tokens(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Score lexical (BM25-lite) d'un schéma de nœud pour une requête.
fn score_node(schema: &NodeSchema, terms: &[String]) -> usize {
    let haystack = format!("{} {} {}", schema.id, schema.label, schema.description).to_lowercase();
    terms.iter().filter(|t| haystack.contains(t.as_str())).count()
}

/// Termes normalisés d'une requête.
fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_alphanumeric())
        .map(|s| s.to_lowercase())
        .filter(|s| s.len() >= 2)
        .collect()
}

/// Retrieval lexical : descriptions des nœuds pertinents dans un budget de tokens.
pub fn retrieve_nodes(query: &str, max_tokens: usize) -> Vec<String> {
    let terms = query_terms(query);
    if terms.is_empty() {
        return Vec::new();
    }
    let schemas = kvl_runtime::builtin_node_schemas();
    let mut scored: Vec<(usize, &NodeSchema)> = schemas
        .iter()
        .map(|s| (score_node(s, &terms), s))
        .filter(|(score, _)| *score > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));

    let mut out = Vec::new();
    let mut used = 0usize;
    for (_, schema) in scored {
        let line = format!("{} — {}", schema.id, schema.description);
        let tokens = estimate_tokens(&line);
        if used + tokens > max_tokens {
            break;
        }
        used += tokens;
        out.push(line);
    }
    out
}

pub struct PromptAssembler;

impl PromptAssembler {
    /// Assemble le prompt minimal (compatibilité) : constitution + schéma + nœuds + intention.
    pub fn assemble(provider: AiProvider, intent: &str) -> AssembledPrompt {
        Self::assemble_full(provider, intent, None, None)
    }

    /// Assemble le prompt complet en respectant le budget du fournisseur.
    pub fn assemble_full(
        provider: AiProvider,
        intent: &str,
        context: Option<&str>,
        dictionary: Option<&AiDictionary>,
    ) -> AssembledPrompt {
        let profile = ProviderProfile::for_provider(provider);

        let mut mandatory: Vec<PromptBlock> = vec![
            PromptBlock {
                label: "CONSTITUTION",
                text: CONSTITUTION.to_string(),
                drop_order: 0,
            },
            PromptBlock {
                label: "SCHEMA",
                text: SCHEMA_HINT.to_string(),
                drop_order: 0,
            },
        ];

        let mut optional: Vec<PromptBlock> = Vec::new();

        let node_budget = (profile.budget_tokens / 4).min(2_000);
        let node_lines = retrieve_nodes(intent, node_budget);
        if !node_lines.is_empty() {
            optional.push(PromptBlock {
                label: "NODES",
                text: format!("Nœuds KVL utiles :\n{}", node_lines.join("\n")),
                drop_order: 1,
            });
        }

        if let Some(ctx) = context {
            optional.push(PromptBlock {
                label: "CONTEXT",
                text: ctx.to_string(),
                drop_order: 2,
            });
        }

        if let Some(dict) = dictionary {
            for name in ["2d-platformer-boss.json", "3d-arena.json"] {
                if let Ok(content) = dict.read_file(&format!("examples/{name}")) {
                    optional.push(PromptBlock {
                        label: "EXAMPLE",
                        text: format!("Exemple {name} :\n{content}"),
                        drop_order: 3,
                    });
                }
            }
        }

        mandatory.push(PromptBlock {
            label: "INTENT",
            text: format!("Intention : {intent}"),
            drop_order: 0,
        });

        let mandatory_tokens: usize = mandatory.iter().map(|b| estimate_tokens(&b.text)).sum();
        let mut budget_left = profile.budget_tokens.saturating_sub(mandatory_tokens);

        let mut sorted_optional = optional;
        sorted_optional.sort_by_key(|b| b.drop_order);

        let mut chosen: Vec<PromptBlock> = mandatory;
        for block in sorted_optional {
            let tokens = estimate_tokens(&block.text);
            if tokens <= budget_left {
                budget_left -= tokens;
                chosen.push(block);
            }
        }

        let text = chosen
            .iter()
            .map(|b| format!("[{}]\n{}\n", b.label, b.text))
            .collect::<Vec<_>>()
            .join("\n");
        let included_blocks = chosen.iter().map(|b| b.label).collect();
        let estimated_tokens = estimate_tokens(&text);
        let hash = hash_text(text.as_bytes());

        AssembledPrompt {
            text,
            hash,
            estimated_tokens,
            dictionary_version: DICTIONARY_VERSION,
            included_blocks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assemble_is_deterministic_and_traced() {
        let a = PromptAssembler::assemble(AiProvider::LmStudio, "Crée un niveau plateforme");
        let b = PromptAssembler::assemble(AiProvider::LmStudio, "Crée un niveau plateforme");
        assert_eq!(a.text, b.text);
        assert_eq!(a.hash, b.hash);
        assert!(!a.hash.is_empty());
        assert!(a.estimated_tokens > 0);
        assert_eq!(a.dictionary_version, DICTIONARY_VERSION);
    }

    #[test]
    fn local_prompt_stays_within_budget() {
        let a = PromptAssembler::assemble(AiProvider::Ollama, "Crée un niveau plateforme");
        assert!(
            a.estimated_tokens <= 8000,
            "budget local dépassé : {} tokens",
            a.estimated_tokens
        );
    }

    #[test]
    fn provider_profiles_have_expected_budgets() {
        assert_eq!(ProviderProfile::for_provider(AiProvider::Ollama).budget_tokens, 8_000);
        assert_eq!(ProviderProfile::for_provider(AiProvider::Claude).budget_tokens, 32_000);
        assert_eq!(ProviderProfile::for_provider(AiProvider::OfflineLocal).budget_tokens, 0);
    }

    #[test]
    fn retrieve_nodes_prefers_boss_terms() {
        let nodes = retrieve_nodes("boss nova", 500);
        assert!(!nodes.is_empty());
        let joined = nodes.join("\n").to_lowercase();
        assert!(joined.contains("boss"), "doit inclure des nœuds boss");
    }

    #[test]
    fn assemble_drops_optional_blocks_when_over_budget() {
        let huge_context = "contexte ".repeat(9000);
        let a = PromptAssembler::assemble_full(AiProvider::LmStudio, "boss", Some(&huge_context), None);

        assert!(a.included_blocks.contains(&"CONSTITUTION"));
        assert!(a.included_blocks.contains(&"INTENT"));
        assert!(
            !a.included_blocks.contains(&"CONTEXT"),
            "le contexte optionnel doit être retiré en cas de dépassement"
        );
        assert!(a.estimated_tokens <= 8_000);
    }
}
