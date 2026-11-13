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

// ============================================================================
// Prompt micro : édition ciblée, 1-3 ops, sans la scène entière
// ============================================================================

/// Constitution du mode micro : une intention = une édition minimale.
pub const CONSTITUTION_MICRO: &str = "Règles immuables : une intention = une édition minimale \
sur la cible fournie. Réponds uniquement en JSON brut, sans markdown ni commentaire. \
Utilise seulement les ops listées et les champs éditables fournis. Jamais de code généré, \
jamais de création de scène, jamais d'op destructive sans nécessité. Si la cible ou la valeur \
est ambiguë, renvoie needs_clarification au lieu d'inventer. Le LLM propose, le moteur dispose.";

/// Ops de mutation exposées au prompt micro (sélection du schéma généré, pas une copie).
pub const MUTATION_OPS: &[&str] = &[
    "update_entity",
    "delete_entity",
    "duplicate_entity",
    "set_transform_2d",
    "set_transform_3d",
    "set_entity_property",
    "add_component",
    "remove_component",
    "set_kvl_node_param",
    "delete_kvl_node",
    "remove_connection",
    "set_scene_setting",
    "batch",
];

/// Budget du prompt micro (tokens).
pub const MICRO_BUDGET_LOCAL: usize = 3_000;
pub const MICRO_BUDGET_CLOUD: usize = 8_000;

/// Estimation dense du nombre de tokens (JSON sans espaces) : `chars / 4`.
///
/// `estimate_tokens` compte les mots ; sur du JSON compact il sous-estimerait d'un facteur
/// important, d'où cette variante pour le mode micro.
pub fn estimate_tokens_dense(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

/// Compacts des ops de mutation, **dérivés du schéma généré** par `schemars`.
///
/// Aucune liste de champs n'est recopiée à la main : si `AiOp` change, cette référence suit.
pub fn mutation_ops_reference() -> String {
    let Ok(value) = serde_json::to_value(schemars::schema_for!(crate::ai_plan::AiOp)) else {
        return String::new();
    };
    let Some(variants) = value.get("oneOf").and_then(|node| node.as_array()) else {
        return String::new();
    };
    let mut lines = Vec::new();
    for variant in variants {
        let Some(name) = variant
            .pointer("/properties/op/const")
            .and_then(|node| node.as_str())
        else {
            continue;
        };
        if !MUTATION_OPS.contains(&name) {
            continue;
        }
        let mut fields: Vec<(String, String)> = Vec::new();
        if let Some(properties) = variant.pointer("/properties").and_then(|node| node.as_object()) {
            for (key, property) in properties {
                if key == "op" {
                    continue;
                }
                fields.push((key.clone(), schema_type_label(property)));
            }
        }
        // `serde_json` trie les clés : on rétablit un ordre de lecture utile au modèle.
        fields.sort_by(|a, b| {
            let rank = |name: &str| match name {
                "target" | "targets" => 0,
                "patch" => 1,
                _ => 2,
            };
            rank(&a.0).cmp(&rank(&b.0)).then_with(|| a.0.cmp(&b.0))
        });
        let rendered: Vec<String> = fields
            .iter()
            .map(|(key, kind)| format!("{key}: {kind}"))
            .collect();
        lines.push(format!("{name}({})", rendered.join(", ")));
    }
    lines.join("\n")
}

fn schema_type_label(property: &serde_json::Value) -> String {
    if let Some(reference) = property.get("$ref").and_then(|node| node.as_str()) {
        return reference.rsplit('/').next().unwrap_or(reference).to_string();
    }
    if let Some(kind) = property.get("type").and_then(|node| node.as_str()) {
        return kind.to_string();
    }
    if property.get("anyOf").is_some() || property.get("oneOf").is_some() {
        return "any".to_string();
    }
    "value".to_string()
}

/// Résumé compact d'une entité pour le bloc `TARGET`.
pub fn entity_summary(entity: &crate::entity::Entity) -> String {
    let mut parts = vec![format!("#{} {} (tag {})", entity.id, entity.name, entity.tag)];
    parts.push(format!(
        "pos({:.0},{:.0}) taille {:.0}×{:.0}",
        entity.transform.x, entity.transform.y, entity.transform.w, entity.transform.h
    ));
    if let Some(renderable) = &entity.renderable {
        parts.push(format!(
            "couleur #{:06X} z{} visible {}",
            renderable.color & 0x00FF_FFFF,
            renderable.z_index,
            renderable.visible
        ));
    }
    if let Some(movement) = &entity.movement {
        parts.push(format!("vitesse {:.0}", movement.speed));
    }
    if let Some(health) = &entity.health {
        parts.push(format!("vie {:.0}/{:.0}", health.current, health.max));
    }
    if let Some(collider) = &entity.collider {
        parts.push(format!("collider {:.0}×{:.0}", collider.w, collider.h));
    }
    if let Some(transform) = &entity.transform3d {
        parts.push(format!(
            "3D pos({:.1},{:.1},{:.1}) échelle({:.1},{:.1},{:.1})",
            transform.position.x, transform.position.y, transform.position.z,
            transform.scale.x, transform.scale.y, transform.scale.z
        ));
    }
    if let Some(renderable) = &entity.renderable3d {
        parts.push(format!(
            "mesh {} matériau {}",
            crate::ai_edit::mesh_debug(&renderable.mesh),
            crate::ai_edit::material_debug(&renderable.material)
        ));
    }
    if !entity.is_active {
        parts.push("inactif".to_string());
    }
    parts.join(" · ")
}

/// Voisins utiles : les 3 entités les plus proches de la cible (jamais la scène entière).
pub fn neighbors(project: &crate::project::KaractiProject, targets: &[usize], limit: usize) -> Vec<String> {
    let Some(anchor) = targets
        .first()
        .and_then(|id| project.scene.entities.iter().find(|entity| entity.id == *id))
    else {
        return Vec::new();
    };
    let (ax, ay) = (anchor.transform.center_x(), anchor.transform.center_y());
    let mut candidates: Vec<(f32, &crate::entity::Entity)> = project
        .scene
        .entities
        .iter()
        .filter(|entity| !targets.contains(&entity.id))
        .map(|entity| {
            let dx = entity.transform.center_x() - ax;
            let dy = entity.transform.center_y() - ay;
            (dx * dx + dy * dy, entity)
        })
        .collect();
    candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    candidates
        .into_iter()
        .take(limit)
        .map(|(_, entity)| {
            format!(
                "#{} {} (tag {}, pos {:.0},{:.0})",
                entity.id, entity.name, entity.tag, entity.transform.x, entity.transform.y
            )
        })
        .collect()
}

/// Bloc `TARGET` : la cible résolue, jamais la scène.
fn target_block(project: &crate::project::KaractiProject, ctx: &crate::ai_edit::EditContext) -> String {
    use crate::ai_edit::{TargetKind, TargetRef, resolve_target};

    match resolve_target(project, ctx, &TargetRef::Selected) {
        Ok(resolved) if resolved.kind == TargetKind::Entity => {
            let mut lines: Vec<String> = resolved
                .entity_ids
                .iter()
                .filter_map(|id| project.scene.entities.iter().find(|entity| entity.id == *id))
                .map(entity_summary)
                .collect();
            if lines.is_empty() {
                lines.push("aucune sélection".to_string());
            }
            format!(
                "Cible : {} ({} élément(s))\n{}",
                resolved.label,
                resolved.entity_ids.len(),
                lines.join("\n")
            )
        }
        Ok(resolved) if resolved.kind == TargetKind::KvlNode => {
            let detail = resolved
                .kvl_graph
                .as_ref()
                .zip(resolved.kvl_node.as_ref())
                .and_then(|(graph, node)| {
                    project
                        .kvl_graphs
                        .get(graph)
                        .and_then(|graph| graph.nodes.get(node))
                        .map(|node| format!("nœud {} « {} » · pins {} in / {} out", node.id, node.name, node.inputs.len(), node.outputs.len()))
                })
                .unwrap_or_else(|| "nœud KVL".to_string());
            format!("Cible : {}\n{detail}", resolved.label)
        }
        Ok(resolved) => format!("Cible : {}", resolved.label),
        Err(error) => format!("Cible : aucune sélection ({error})"),
    }
}

/// Assemble le prompt micro d'édition (1-3 ops) pour le fournisseur choisi.
pub fn assemble_micro(
    provider: AiProvider,
    project: &crate::project::KaractiProject,
    ctx: &crate::ai_edit::EditContext,
    instruction: &str,
) -> AssembledPrompt {
    let budget = match provider {
        AiProvider::Ollama | AiProvider::LmStudio | AiProvider::OfflineLocal => MICRO_BUDGET_LOCAL,
        _ => MICRO_BUDGET_CLOUD,
    };

    let targets = crate::ai_edit::resolve_target(project, ctx, &crate::ai_edit::TargetRef::Selected)
        .map(|resolved| resolved.entity_ids)
        .unwrap_or_default();

    let mut mandatory: Vec<PromptBlock> = vec![
        PromptBlock {
            label: "CONSTITUTION",
            text: CONSTITUTION_MICRO.to_string(),
            drop_order: 0,
        },
        PromptBlock {
            label: "OP_SCHEMA",
            text: format!("Ops disponibles (résumé du schéma généré) :\n{}", mutation_ops_reference()),
            drop_order: 0,
        },
        PromptBlock {
            label: "TARGET",
            text: target_block(project, ctx),
            drop_order: 0,
        },
    ];

    let mut optional: Vec<PromptBlock> = vec![
        PromptBlock {
            label: "CAPABILITIES",
            text: format!(
                "Champs éditables par entité : {}. Bornes : taille/vie > 0, vitesse ≥ 0, \
                 couleur 0x00RRGGBB. Composants ajoutables : collider, movement, health, vision, \
                 projectile, boss, transform3d, renderable3d.",
                crate::ai_edit::EDITABLE_ENTITY_PATHS.join(", ")
            ),
            drop_order: 1,
        },
    ];

    let nearby = neighbors(project, &targets, 3);
    if !nearby.is_empty() {
        optional.push(PromptBlock {
            label: "NEIGHBORS",
            text: format!("Voisins proches :\n{}", nearby.join("\n")),
            drop_order: 2,
        });
    }

    optional.push(PromptBlock {
        label: "EXAMPLE",
        text: "Exemple : {\"ops\":[{\"op\":\"update_entity\",\"target\":\"selected\",\
              \"patch\":{\"color\":14483750,\"speed\":160.0}}],\"rationale\":\"Le boss est terne \
              et lent ; je propose un rouge rubis et une vitesse de 160.\",\"risk\":\"safe\"}"
            .to_string(),
        drop_order: 3,
    });

    mandatory.push(PromptBlock {
        label: "INSTRUCTION",
        text: format!("Instruction : {instruction}"),
        drop_order: 0,
    });
    mandatory.push(PromptBlock {
        label: "OUTPUT",
        text: "Sortie attendue : {\"ops\":[<op>…],\"rationale\":\"une phrase\",\
               \"risk\":\"safe|confirm|destructive\"} en JSON brut. \
               Si la cible est ambiguë : {\"needs_clarification\":\"question\"}."
            .to_string(),
        drop_order: 0,
    });

    let mandatory_tokens: usize = mandatory.iter().map(|block| estimate_tokens_dense(&block.text)).sum();
    let mut budget_left = budget.saturating_sub(mandatory_tokens);

    optional.sort_by_key(|block| block.drop_order);
    let mut chosen = mandatory;
    for block in optional {
        let tokens = estimate_tokens_dense(&block.text);
        if tokens <= budget_left {
            budget_left -= tokens;
            chosen.push(block);
        }
    }

    let text = chosen
        .iter()
        .map(|block| format!("[{}]\n{}\n", block.label, block.text))
        .collect::<Vec<_>>()
        .join("\n");
    let included_blocks = chosen.iter().map(|block| block.label).collect();
    AssembledPrompt {
        hash: hash_text(text.as_bytes()),
        estimated_tokens: estimate_tokens_dense(&text),
        text,
        dictionary_version: DICTIONARY_VERSION,
        included_blocks,
    }
}

#[cfg(test)]
mod micro_tests {
    use super::*;
    use crate::ai_edit::{EditContext, TargetRef};
    use crate::entity::{Entity, Movement2D, Renderable2D, Transform2D};
    use crate::recipe::GameGenre;

    fn project() -> crate::project::KaractiProject {
        let mut project = crate::project::KaractiProject::new_empty("Test", "auteur", GameGenre::Platformer);
        project.scene.entities.clear();
        for (index, (name, tag, x, y)) in [
            ("Boss", "enemy", 100.0, 100.0),
            ("Pièce", "collectible", 300.0, 50.0),
            ("Mur", "wall", 120.0, 140.0),
            ("Porte", "door", 500.0, 300.0),
            ("Piège", "hazard", 900.0, 700.0),
        ]
        .iter()
        .enumerate()
        {
            let mut entity = Entity::new(
                index + 1,
                *name,
                *tag,
                Transform2D::new(*x, *y, 40.0, 40.0),
            );
            entity.renderable = Some(Renderable2D::rect(0x00475569, 5));
            if index == 0 {
                entity.movement = Some(Movement2D::new(110.0));
            }
            project.scene.entities.push(entity);
        }
        project
    }

    fn ctx(ids: &[usize]) -> EditContext {
        EditContext::with_selection(
            &project(),
            ids.iter().map(|id| TargetRef::EntityId(*id)).collect(),
        )
    }

    #[test]
    fn ops_reference_is_generated_from_the_real_schema() {
        let reference = mutation_ops_reference();
        let lines: Vec<&str> = reference.lines().collect();
        assert_eq!(lines.len(), MUTATION_OPS.len(), "une ligne par op de mutation");
        assert!(reference.contains("update_entity(target: TargetRef, patch: EntityPatch)"));
        assert!(reference.contains("set_transform_2d("));
        assert!(reference.contains("batch(ops:"));
    }

    #[test]
    fn micro_prompt_is_deterministic_and_targeted() {
        let project = project();
        let context = ctx(&[1]);
        let first = assemble_micro(AiProvider::LmStudio, &project, &context, "mets-le en rouge");
        let second = assemble_micro(AiProvider::LmStudio, &project, &context, "mets-le en rouge");
        assert_eq!(first.text, second.text);
        assert_eq!(first.hash, second.hash);
        assert!(first.text.contains("Boss"), "la cible doit être dans le prompt");
        assert!(first.text.contains("pos(100,100)"));
        assert!(first.text.contains("mets-le en rouge"));
    }

    #[test]
    fn micro_prompt_fits_the_local_budget() {
        let project = project();
        let context = ctx(&[1]);
        let prompt = assemble_micro(AiProvider::Ollama, &project, &context, "plus rapide et rouge");
        assert!(
            prompt.estimated_tokens <= MICRO_BUDGET_LOCAL,
            "budget local dépassé : {} tokens",
            prompt.estimated_tokens
        );
        assert!(prompt.included_blocks.contains(&"CONSTITUTION"));
        assert!(prompt.included_blocks.contains(&"TARGET"));
        assert!(prompt.included_blocks.contains(&"OUTPUT"));
    }

    #[test]
    fn micro_prompt_without_selection_says_so_instead_of_inventing() {
        let project = project();
        let context = EditContext::for_project(&project);
        let prompt = assemble_micro(AiProvider::LmStudio, &project, &context, "mets-le en rouge");
        assert!(prompt.text.contains("aucune sélection"), "obtenu : {}", prompt.text);
    }

    #[test]
    fn neighbors_are_limited_and_exclude_the_target() {
        let project = project();
        let nearby = neighbors(&project, &[1], 2);
        assert_eq!(nearby.len(), 2);
        assert!(!nearby.iter().any(|line| line.contains("#1 ")));
        // Le plus proche du Boss (100,100) est le Mur (120,140), pas le Piège (900,700).
        assert!(nearby[0].contains("Mur"));
    }

    #[test]
    fn micro_prompt_never_contains_the_whole_scene() {
        let project = project();
        let context = ctx(&[1]);
        let prompt = assemble_micro(AiProvider::LmStudio, &project, &context, "rouge");
        assert!(prompt.text.contains("Boss"), "la cible est présente");
        // Les 3 voisins les plus proches sont Mur, Pièce et Porte : le Piège est exclu.
        assert!(!prompt.text.contains("Piège"), "les voisins lointains sont exclus");
    }
}
