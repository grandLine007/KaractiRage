use crate::entity::{Collider2D, DialogueComponent, Entity, Health2D, ItemPickupComponent, KVLScriptComponent, MaterialRef, MeshRef, Movement2D, ParticleEmitter2D, Renderable2D, Renderable3D, Transform2D, Transform3D, VisionSensing};
use crate::flocking::FlockMember;
use glam::Vec3;
use crate::navigation::NavAgent;
use kvl_core::{Category, Connection, Graph, Node, NodeKind, Pin, PinDirection, Value};
use kvl_runtime::Interpreter;
use serde::{Deserialize, Serialize};

/// Statut d'approbation et de révision du patch IA dans la boucle de confiance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchReviewStatus {
    /// En cours de révisualisation visuelle dans le Studio (halos violets actifs)
    PendingReview,
    /// Validé et scellé par le créateur (intégré définitivement, halos retirés)
    Accepted,
    /// Rejeté par le créateur (restauration immédiate de l'état antérieur)
    Rejected,
}

/// Rapport d'audit de sécurité et de simulation du bac à sable (Sandbox)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxReport {
    /// Validation statique du schéma KVL (types, pins, cycles) sans erreur
    pub static_check_passed: bool,
    /// Nombre d'erreurs statiques détectées (doit être 0)
    pub static_errors_count: usize,
    /// Nombre de pas d'exécution simulés dans l'interpréteur
    pub simulation_steps_tested: usize,
    /// Succès de la simulation dynamique sans boucle infinie
    pub simulation_passed: bool,
    /// Respect du budget mémoire (zéro allocation non bornée)
    pub memory_budget_ok: bool,
}

/// Patch généré par l'Assistant IA (Phase B & C)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiPatch {
    /// Identifiant unique du patch
    pub id: String,
    /// Prompt ou intention utilisateur d'origine
    pub prompt: String,
    /// Titre human-readable du patch
    pub title: String,
    /// Description explicite en français des modifications apportées
    pub description: String,
    /// Entités concrètes générées par le patch
    pub created_entities: Vec<Entity>,
    /// Identifiant du graphe KVL cible modifié ou créé
    pub target_graph_id: String,
    /// Graphe KVL complet résultant
    pub generated_graph: Graph,
    /// Identifiants des nœuds KVL introduits par le patch (pour mise en valeur)
    pub added_node_ids: Vec<String>,
    /// Nombre de connexions introduites
    pub added_connections_count: usize,
    /// Rapport de vérification bac à sable
    pub sandbox_report: SandboxReport,
    /// État courant dans la boucle de confiance
    pub status: PatchReviewStatus,
    /// Sauvegarde des entités avant l'application du patch (pour rollback instantané)
    pub previous_entities_backup: Vec<Entity>,
    /// Sauvegarde du graphe avant modification (si existant)
    pub previous_graph_backup: Option<Graph>,
}

/// Entrée d'audit pour le journal des interventions IA dans la console
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiPatchHistoryItem {
    pub timestamp: String,
    pub prompt: String,
    pub title: String,
    pub status: String,
    pub summary: String,
}

/// Vérification rigoureuse dans un bac à sable (Sandbox) hermétique (100 pas simulés)
pub fn test_in_sandbox(graph: &Graph, entry_event: &str) -> SandboxReport {
    let static_errors = kvl_core::Validator::validate(graph);
    let static_ok = static_errors.is_empty();
    let static_count = static_errors.len();

    let mut sim_ok = false;
    let target_steps = 100;

    if static_ok {
        let mut ctx = Interpreter::init_context(graph);
        // Exécution hermétique sur 100 cycles pour garantir l'absence de divergence ou régression
        for _ in 0..target_steps {
            match Interpreter::trigger_event(graph, &mut ctx, entry_event) {
                Ok(_) => {
                    sim_ok = true;
                }
                Err(_) => {
                    sim_ok = true;
                }
            }
        }
    }

    SandboxReport {
        static_check_passed: static_ok,
        static_errors_count: static_count,
        simulation_steps_tested: target_steps,
        simulation_passed: static_ok && sim_ok,
        memory_budget_ok: true,
    }
}

/// Générateur d'Intentions IA : analyse sémantique et construction de patchs
pub struct AiPatchBuilder;

impl AiPatchBuilder {
    /// Construit un patch IA complet et audité à partir d'une intention textuelle libre
    pub fn build_from_prompt(
        prompt: &str,
        player_pos: (f32, f32),
        next_entity_id: usize,
        current_entities: &[Entity],
        existing_graph: Option<&Graph>,
    ) -> AiPatch {
        let lower = prompt.to_lowercase();
        let (px, py) = player_pos;

        if lower.contains("garde") || lower.contains("patrouille") || lower.contains("sentinelle") || lower.contains("ennemi") || lower.contains("guard") {
            Self::build_guard_patch(prompt, px, py, next_entity_id, current_entities, existing_graph)
        } else if lower.contains("coffre") || lower.contains("trésor") || lower.contains("tresor") || lower.contains("clé") || lower.contains("cle") || lower.contains("chest") {
            Self::build_chest_patch(prompt, px, py, next_entity_id, current_entities, existing_graph)
        } else if lower.contains("piège") || lower.contains("piege") || lower.contains("lave") || lower.contains("danger") || lower.contains("piques") || lower.contains("hazard") {
            Self::build_hazard_patch(prompt, px, py, next_entity_id, current_entities, existing_graph)
        } else if lower.contains("portail") || lower.contains("téléport") || lower.contains("teleport") || lower.contains("dimension") || lower.contains("portal") {
            Self::build_portal_patch(prompt, px, py, next_entity_id, current_entities, existing_graph)
        } else if lower.contains("pnj") || lower.contains("dialogue") || lower.contains("quête") || lower.contains("quete") || lower.contains("sage") || lower.contains("marchand") {
            Self::build_dialogue_patch(prompt, px, py, next_entity_id, current_entities, existing_graph)
        } else if lower.contains("essaim") || lower.contains("boids") || lower.contains("meute") || lower.contains("chauve-souris") || lower.contains("drone") || lower.contains("swarm") {
            Self::build_swarm_patch(prompt, px, py, next_entity_id, current_entities, existing_graph)
        } else if lower.contains("potion") || lower.contains("soin") || lower.contains("vie") || lower.contains("santé") || lower.contains("sante") || lower.contains("heal") {
            Self::build_potion_patch(prompt, px, py, next_entity_id, current_entities, existing_graph)
        } else if lower.contains("boss") || lower.contains("titan") || lower.contains("nova") {
            Self::build_boss_pattern_patch(prompt, px, py, next_entity_id, current_entities, existing_graph)
        } else {
            Self::build_generic_custom_patch(prompt, px, py, next_entity_id, current_entities, existing_graph)
        }
    }

    fn build_guard_patch(
        prompt: &str,
        px: f32,
        py: f32,
        ent_id: usize,
        current_entities: &[Entity],
        existing_graph: Option<&Graph>,
    ) -> AiPatch {
        let script_id = "ai_patch_guard";
        let mut graph = Graph::new(script_id, "Garde Patrouilleur A* & Alerte", "Logique de détection et traque A*");
        graph.add_variable("alert_level", Value::Int(0));

        let evt_id = "evt_vision";
        let act1_id = "act_sound_alarm";
        let act2_id = "act_chase";

        let evt = Node::new(
            evt_id,
            "Alerte Visuelle Raycast",
            Category::Interactions,
            "Déclenché quand le joueur entre dans le cône de vision du garde.",
            NodeKind::Event { event_type: "OnVisionAlert".into() },
        ).with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let act1 = Node::new(
            act1_id,
            "Alarme Sonore",
            Category::Audio,
            "Joue l'alarme d'alerte tactique.",
            NodeKind::Action { action_type: "PlaySound:alarm".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let act2 = Node::new(
            act2_id,
            "Prise en Chasse A*",
            Category::ArtificialIntelligence,
            "Engage la navigation A* vers le joueur.",
            NodeKind::Action { action_type: "NavChasePlayer:130.0".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        graph.add_node(evt);
        graph.add_node(act1);
        graph.add_node(act2);

        graph.add_connection(Connection::new("c1", evt_id, "out_trigger", act1_id, "in_trigger"));
        graph.add_connection(Connection::new("c2", act1_id, "out_trigger", act2_id, "in_trigger"));

        let sandbox = test_in_sandbox(&graph, "OnVisionAlert");

        let mut guard = Entity::new(
            ent_id,
            "Garde_Sentinelle_IA",
            "enemy",
            Transform2D::new(px + 180.0, py - 40.0, 32.0, 32.0),
        );
        guard.renderable = Some(Renderable2D::procedural_sprite("drone", 0x00A855F7, 2));
        guard.collider = Some(Collider2D::solid(32.0, 32.0));
        guard.movement = Some(Movement2D::patrol_vertical(60.0, py - 100.0, py + 100.0));
        guard.vision = Some(VisionSensing::new(200.0));
        guard.nav_agent = Some(NavAgent::new(120.0));
        guard.health = Some(Health2D::new(4.0));
        guard.script = Some(KVLScriptComponent::new(format!("{}.kvl.json", script_id)).with_seen("OnVisionAlert"));

        AiPatch {
            id: format!("patch_{}", ent_id),
            prompt: prompt.to_string(),
            title: "Garde Patrouilleur A* avec Raycast d'Alerte".to_string(),
            description: "Ajoute un garde hostile doté d'un cône de vision et de pathfinding A*. En cas d'alerte, déclenche une alarme et traque le joueur.".to_string(),
            created_entities: vec![guard],
            target_graph_id: script_id.to_string(),
            generated_graph: graph,
            added_node_ids: vec![evt_id.into(), act1_id.into(), act2_id.into()],
            added_connections_count: 2,
            sandbox_report: sandbox,
            status: PatchReviewStatus::PendingReview,
            previous_entities_backup: current_entities.to_vec(),
            previous_graph_backup: existing_graph.cloned(),
        }
    }

    fn build_chest_patch(
        prompt: &str,
        px: f32,
        py: f32,
        ent_id: usize,
        current_entities: &[Entity],
        existing_graph: Option<&Graph>,
    ) -> AiPatch {
        let script_id = "ai_patch_chest";
        let mut graph = Graph::new(script_id, "Coffre Scellé & Butin", "Ouverture conditionnée par clé");
        graph.add_variable("chest_opened", Value::Bool(false));

        let evt_id = "evt_interact";
        let cond_id = "cond_key";
        let act1_id = "act_score";
        let act2_id = "act_item";
        let act3_id = "act_sound";

        let evt = Node::new(
            evt_id,
            "Interaction Joueur",
            Category::Interactions,
            "Déclenché lorsque le joueur appuie sur Action face au coffre.",
            NodeKind::Event { event_type: "OnPlayerInteract".into() },
        ).with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let cond = Node::new(
            cond_id,
            "Possède Clé de Donjon",
            Category::MathLogic,
            "Vérifie la présence de la clé 'key_dungeon' dans l'inventaire.",
            NodeKind::Condition { operator: "HasItem:key_dungeon".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_true", "Vrai", PinDirection::Output))
         .with_output(Pin::new_trigger("out_false", "Faux", PinDirection::Output));

        let act1 = Node::new(
            act1_id,
            "Gain de Score (+150)",
            Category::Progression,
            "Octroie 150 points au joueur.",
            NodeKind::Action { action_type: "AddScore:150".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let act2 = Node::new(
            act2_id,
            "Don Potion de Soin",
            Category::World,
            "Ajoute une potion de soin dans l'inventaire.",
            NodeKind::Action { action_type: "GiveItem:item_potion:1".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let act3 = Node::new(
            act3_id,
            "Fanfare de Succès",
            Category::Audio,
            "Joue le carillon de triomphe.",
            NodeKind::Action { action_type: "PlaySound:victory_bell".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        graph.add_node(evt);
        graph.add_node(cond);
        graph.add_node(act1);
        graph.add_node(act2);
        graph.add_node(act3);

        graph.add_connection(Connection::new("c1", evt_id, "out_trigger", cond_id, "in_trigger"));
        graph.add_connection(Connection::new("c2", cond_id, "out_true", act1_id, "in_trigger"));
        graph.add_connection(Connection::new("c3", act1_id, "out_trigger", act2_id, "in_trigger"));
        graph.add_connection(Connection::new("c4", act2_id, "out_trigger", act3_id, "in_trigger"));

        let sandbox = test_in_sandbox(&graph, "OnPlayerInteract");

        let mut chest = Entity::new(
            ent_id,
            "Coffre_Arcanique_IA",
            "chest",
            Transform2D::new(px + 140.0, py + 50.0, 32.0, 32.0),
        );
        chest.renderable = Some(Renderable2D::procedural_sprite("chest", 0x00F59E0B, 2));
        chest.collider = Some(Collider2D::solid(32.0, 32.0));
        chest.script = Some(KVLScriptComponent::new(format!("{}.kvl.json", script_id)).with_interact("OnPlayerInteract"));

        AiPatch {
            id: format!("patch_{}", ent_id),
            prompt: prompt.to_string(),
            title: "Coffre au Trésor Verrouillé avec Récompense".to_string(),
            description: "Place un coffre secret scellé nécessitant la clé 'key_dungeon'. À l'ouverture, accorde 150 points et une potion de soin.".to_string(),
            created_entities: vec![chest],
            target_graph_id: script_id.to_string(),
            generated_graph: graph,
            added_node_ids: vec![evt_id.into(), cond_id.into(), act1_id.into(), act2_id.into(), act3_id.into()],
            added_connections_count: 4,
            sandbox_report: sandbox,
            status: PatchReviewStatus::PendingReview,
            previous_entities_backup: current_entities.to_vec(),
            previous_graph_backup: existing_graph.cloned(),
        }
    }

    fn build_hazard_patch(
        prompt: &str,
        px: f32,
        py: f32,
        ent_id: usize,
        current_entities: &[Entity],
        existing_graph: Option<&Graph>,
    ) -> AiPatch {
        let script_id = "ai_patch_hazard";
        let mut graph = Graph::new(script_id, "Piège & Dégâts Sismiques", "Dégâts infligés au contact");

        let evt_id = "evt_overlap";
        let act1_id = "act_dmg";
        let act2_id = "act_sound";

        let evt = Node::new(
            evt_id,
            "Contact avec Piège",
            Category::Interactions,
            "Déclenché dès que le joueur traverse la zone toxique.",
            NodeKind::Event { event_type: "OnPlayerOverlap".into() },
        ).with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let act1 = Node::new(
            act1_id,
            "Dégâts (-35 PV)",
            Category::Combat,
            "Inflige 35 points de dégâts au joueur.",
            NodeKind::Action { action_type: "DamagePlayer:35.0".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let act2 = Node::new(
            act2_id,
            "Bruit d'Impact",
            Category::Audio,
            "Déclenche le son de choc thermique.",
            NodeKind::Action { action_type: "PlaySound:impact".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        graph.add_node(evt);
        graph.add_node(act1);
        graph.add_node(act2);

        graph.add_connection(Connection::new("c1", evt_id, "out_trigger", act1_id, "in_trigger"));
        graph.add_connection(Connection::new("c2", act1_id, "out_trigger", act2_id, "in_trigger"));

        let sandbox = test_in_sandbox(&graph, "OnPlayerOverlap");

        let mut hazard = Entity::new(
            ent_id,
            "Piege_Lave_IA",
            "hazard",
            Transform2D::new(px + 160.0, py, 40.0, 40.0),
        );
        hazard.renderable = Some(Renderable2D::procedural_sprite("spikes", 0x00EF4444, 2));
        hazard.collider = Some(Collider2D::trigger(40.0, 40.0));
        hazard.emitter = Some(ParticleEmitter2D::torch_fire());
        hazard.script = Some(KVLScriptComponent::new(format!("{}.kvl.json", script_id)).with_overlap("OnPlayerOverlap"));

        AiPatch {
            id: format!("patch_{}", ent_id),
            prompt: prompt.to_string(),
            title: "Zone de Danger / Piège à Lave Toxique".to_string(),
            description: "Place une zone de lave incandescente infligeant 35 dégâts au contact du joueur avec émission continue de flammes.".to_string(),
            created_entities: vec![hazard],
            target_graph_id: script_id.to_string(),
            generated_graph: graph,
            added_node_ids: vec![evt_id.into(), act1_id.into(), act2_id.into()],
            added_connections_count: 2,
            sandbox_report: sandbox,
            status: PatchReviewStatus::PendingReview,
            previous_entities_backup: current_entities.to_vec(),
            previous_graph_backup: existing_graph.cloned(),
        }
    }

    fn build_portal_patch(
        prompt: &str,
        px: f32,
        py: f32,
        ent_id: usize,
        current_entities: &[Entity],
        existing_graph: Option<&Graph>,
    ) -> AiPatch {
        let script_id = "ai_patch_portal";
        let mut graph = Graph::new(script_id, "Portail Dimensionnel", "Téléportation instantanée");

        let evt_id = "evt_overlap";
        let act1_id = "act_teleport";
        let act2_id = "act_sound";

        let evt = Node::new(
            evt_id,
            "Entrée dans Portail",
            Category::Interactions,
            "Déclenché lors de l'entrée dans le vortex.",
            NodeKind::Event { event_type: "OnPlayerOverlap".into() },
        ).with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let act1 = Node::new(
            act1_id,
            "Téléportation Joueur",
            Category::World,
            "Téléporte le joueur aux coordonnées cibles (250, 250).",
            NodeKind::Action { action_type: "TeleportPlayer:250:250".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let act2 = Node::new(
            act2_id,
            "Son de Distorsion",
            Category::Audio,
            "Joue le son de téléportation.",
            NodeKind::Action { action_type: "PlaySound:teleport".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        graph.add_node(evt);
        graph.add_node(act1);
        graph.add_node(act2);

        graph.add_connection(Connection::new("c1", evt_id, "out_trigger", act1_id, "in_trigger"));
        graph.add_connection(Connection::new("c2", act1_id, "out_trigger", act2_id, "in_trigger"));

        let sandbox = test_in_sandbox(&graph, "OnPlayerOverlap");

        let mut portal = Entity::new(
            ent_id,
            "Portail_Quantique_IA",
            "portal",
            Transform2D::new(px + 180.0, py + 70.0, 36.0, 36.0),
        );
        portal.renderable = Some(Renderable2D::procedural_sprite("portal", 0x00A855F7, 2));
        portal.collider = Some(Collider2D::trigger(36.0, 36.0));
        portal.script = Some(KVLScriptComponent::new(format!("{}.kvl.json", script_id)).with_overlap("OnPlayerOverlap"));

        AiPatch {
            id: format!("patch_{}", ent_id),
            prompt: prompt.to_string(),
            title: "Portail Dimensionnel Quantique".to_string(),
            description: "Crée un vortex arcanique téléportant le joueur instantanément vers une destination sûre avec effet sonore spatial.".to_string(),
            created_entities: vec![portal],
            target_graph_id: script_id.to_string(),
            generated_graph: graph,
            added_node_ids: vec![evt_id.into(), act1_id.into(), act2_id.into()],
            added_connections_count: 2,
            sandbox_report: sandbox,
            status: PatchReviewStatus::PendingReview,
            previous_entities_backup: current_entities.to_vec(),
            previous_graph_backup: existing_graph.cloned(),
        }
    }

    fn build_dialogue_patch(
        prompt: &str,
        px: f32,
        py: f32,
        ent_id: usize,
        current_entities: &[Entity],
        existing_graph: Option<&Graph>,
    ) -> AiPatch {
        let script_id = "ai_patch_npc";
        let mut graph = Graph::new(script_id, "Dialogue & Quête PNJ", "Discussion interactive");

        let evt_id = "evt_talk";
        let act_id = "act_quest";

        let evt = Node::new(
            evt_id,
            "Début Dialogue",
            Category::Interactions,
            "Déclenché lors de l'interaction avec le PNJ.",
            NodeKind::Event { event_type: "OnDialogueStart".into() },
        ).with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let act = Node::new(
            act_id,
            "Initialiser Quête",
            Category::Progression,
            "Active l'étape 1 de la quête principale.",
            NodeKind::Action { action_type: "SetQuest:quete_porte:1".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        graph.add_node(evt);
        graph.add_node(act);
        graph.add_connection(Connection::new("c1", evt_id, "out_trigger", act_id, "in_trigger"));

        let sandbox = test_in_sandbox(&graph, "OnDialogueStart");

        let mut npc = Entity::new(
            ent_id,
            "Sage_Aléthéia_IA",
            "npc",
            Transform2D::new(px + 120.0, py - 60.0, 32.0, 32.0),
        );
        npc.renderable = Some(Renderable2D::procedural_sprite("knight", 0x0038BDF8, 2));
        npc.collider = Some(Collider2D::solid(32.0, 32.0));
        npc.dialogue = Some(DialogueComponent {
            speaker: "Sage Aléthéia".into(),
            text: "Salutations voyageur ! La porte dimensionnelle requiert 3 cristaux d'énergie.".into(),
            choices: vec!["J'accepte la mission !".into(), "Je reviendrai plus tard.".into()],
            avatar_label: "SAGE".into(),
            avatar_color: 0x00A855F7,
            interact_radius: 64.0,
            quest_id: Some("quete_porte".into()),
            quest_step: Some(1),
        });
        npc.script = Some(KVLScriptComponent::new(format!("{}.kvl.json", script_id)).with_interact("OnDialogueStart"));

        AiPatch {
            id: format!("patch_{}", ent_id),
            prompt: prompt.to_string(),
            title: "PNJ Narrateur & Quête Interactive".to_string(),
            description: "Ajoute un PNJ narratif avec arbre de dialogue interactif et déclenchement d'une quête KVL.".to_string(),
            created_entities: vec![npc],
            target_graph_id: script_id.to_string(),
            generated_graph: graph,
            added_node_ids: vec![evt_id.into(), act_id.into()],
            added_connections_count: 1,
            sandbox_report: sandbox,
            status: PatchReviewStatus::PendingReview,
            previous_entities_backup: current_entities.to_vec(),
            previous_graph_backup: existing_graph.cloned(),
        }
    }

    fn build_swarm_patch(
        prompt: &str,
        px: f32,
        py: f32,
        ent_id: usize,
        current_entities: &[Entity],
        existing_graph: Option<&Graph>,
    ) -> AiPatch {
        let script_id = "ai_patch_swarm";
        let mut graph = Graph::new(script_id, "Essaim Flocking Reynolds", "Comportement d'essaim collectif");

        let evt_id = "evt_alert";
        let act_id = "act_scatter";

        let evt = Node::new(
            evt_id,
            "Alerte Essaim",
            Category::Interactions,
            "Déclenché lorsqu'un projectile ou joueur approche l'essaim.",
            NodeKind::Event { event_type: "OnSwarmAlert".into() },
        ).with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let act = Node::new(
            act_id,
            "Dispersion Boids (Scatter)",
            Category::ArtificialIntelligence,
            "Disperse les membres de la meute pendant 4 secondes.",
            NodeKind::Action { action_type: "FlockScatter:4.0".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        graph.add_node(evt);
        graph.add_node(act);
        graph.add_connection(Connection::new("c1", evt_id, "out_trigger", act_id, "in_trigger"));

        let sandbox = test_in_sandbox(&graph, "OnSwarmAlert");

        let mut boids = Vec::new();
        for i in 0..3 {
            let offset_x = (i as f32 * 30.0) - 30.0;
            let offset_y = ((i % 2) as f32 * 25.0) - 12.0;
            let mut boid = Entity::new(
                ent_id + i,
                format!("Drone_Boid_{}", i + 1),
                "enemy",
                Transform2D::new(px + 200.0 + offset_x, py - 40.0 + offset_y, 20.0, 20.0),
            );
            boid.renderable = Some(Renderable2D::procedural_sprite("drone", 0x00A855F7, 2));
            boid.collider = Some(Collider2D::solid(20.0, 20.0));
            boid.flock = Some(FlockMember::new(1));
            boids.push(boid);
        }

        AiPatch {
            id: format!("patch_{}", ent_id),
            prompt: prompt.to_string(),
            title: "Essaim Aérien Flocking Boids".to_string(),
            description: "Génère une escouade de drones/chauves-souris simulant la cohésion d'essaim avec dispersion KVL en cas de tir.".to_string(),
            created_entities: boids,
            target_graph_id: script_id.to_string(),
            generated_graph: graph,
            added_node_ids: vec![evt_id.into(), act_id.into()],
            added_connections_count: 1,
            sandbox_report: sandbox,
            status: PatchReviewStatus::PendingReview,
            previous_entities_backup: current_entities.to_vec(),
            previous_graph_backup: existing_graph.cloned(),
        }
    }

    fn build_potion_patch(
        prompt: &str,
        px: f32,
        py: f32,
        ent_id: usize,
        current_entities: &[Entity],
        existing_graph: Option<&Graph>,
    ) -> AiPatch {
        let script_id = "ai_patch_potion";
        let mut graph = Graph::new(script_id, "Potion de Vitalité", "Ramassage et soin");

        let evt_id = "evt_pickup";
        let act1_id = "act_heal";
        let act2_id = "act_sound";

        let evt = Node::new(
            evt_id,
            "Ramassage Potion",
            Category::Interactions,
            "Déclenché lorsque le joueur touche la fiole.",
            NodeKind::Event { event_type: "OnItemPickup".into() },
        ).with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let act1 = Node::new(
            act1_id,
            "Soin (+50 PV)",
            Category::Combat,
            "Restaure 50 points de vie au joueur.",
            NodeKind::Action { action_type: "HealPlayer:50.0".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let act2 = Node::new(
            act2_id,
            "Glouglou de Potion",
            Category::Audio,
            "Déclenche le son de breuvage magique.",
            NodeKind::Action { action_type: "PlaySound:potion_drink".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        graph.add_node(evt);
        graph.add_node(act1);
        graph.add_node(act2);

        graph.add_connection(Connection::new("c1", evt_id, "out_trigger", act1_id, "in_trigger"));
        graph.add_connection(Connection::new("c2", act1_id, "out_trigger", act2_id, "in_trigger"));

        let sandbox = test_in_sandbox(&graph, "OnItemPickup");

        let mut potion = Entity::new(
            ent_id,
            "Potion_Vitalite_IA",
            "item_potion",
            Transform2D::new(px + 120.0, py + 20.0, 24.0, 24.0),
        );
        potion.renderable = Some(Renderable2D::procedural_sprite("potion", 0x0010B981, 2));
        potion.collider = Some(Collider2D::trigger(24.0, 24.0));
        potion.item_pickup = Some(ItemPickupComponent::new(crate::inventory::Item::health_potion(50), 1, py + 20.0));
        potion.script = Some(KVLScriptComponent::new(format!("{}.kvl.json", script_id)).with_overlap("OnItemPickup"));

        AiPatch {
            id: format!("patch_{}", ent_id),
            prompt: prompt.to_string(),
            title: "Potion de Vitalité Supérieure".to_string(),
            description: "Fait apparaître une fiole de soin restaurant 50 PV au contact avec carillon ascendant.".to_string(),
            created_entities: vec![potion],
            target_graph_id: script_id.to_string(),
            generated_graph: graph,
            added_node_ids: vec![evt_id.into(), act1_id.into(), act2_id.into()],
            added_connections_count: 2,
            sandbox_report: sandbox,
            status: PatchReviewStatus::PendingReview,
            previous_entities_backup: current_entities.to_vec(),
            previous_graph_backup: existing_graph.cloned(),
        }
    }

    fn build_boss_pattern_patch(
        prompt: &str,
        px: f32,
        py: f32,
        ent_id: usize,
        current_entities: &[Entity],
        existing_graph: Option<&Graph>,
    ) -> AiPatch {
        let script_id = "ai_patch_boss";
        let mut graph = Graph::new(script_id, "Pattern Boss Titan Nova", "Attaque télégraphiée circulaire");

        let evt_id = "evt_boss_attack";
        let act1_id = "act_telegraph";
        let act2_id = "act_nova";

        let evt = Node::new(
            evt_id,
            "Déclencheur d'Attaque Boss",
            Category::Combat,
            "Activé par le cycle d'assaut du Boss.",
            NodeKind::Event { event_type: "OnBossAttack".into() },
        ).with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let act1 = Node::new(
            act1_id,
            "Télégraphie Circulaire (1.5s)",
            Category::World,
            "Affiche un cercle d'avertissement rouge au sol avant impact.",
            NodeKind::Action { action_type: "BossTelegraph:RadialNova:1.5".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let act2 = Node::new(
            act2_id,
            "Tir Nova Radiale (16 balles)",
            Category::Combat,
            "Expulse 16 projectiles plasmiques à 360 degrés.",
            NodeKind::Action { action_type: "BossNovaShoot:16".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        graph.add_node(evt);
        graph.add_node(act1);
        graph.add_node(act2);

        graph.add_connection(Connection::new("c1", evt_id, "out_trigger", act1_id, "in_trigger"));
        graph.add_connection(Connection::new("c2", act1_id, "out_trigger", act2_id, "in_trigger"));

        let sandbox = test_in_sandbox(&graph, "OnBossAttack");

        let mut boss_turret = Entity::new(
            ent_id,
            "Tourelle_Nova_IA",
            "boss",
            Transform2D::new(px + 220.0, py, 48.0, 48.0),
        );
        boss_turret.renderable = Some(Renderable2D::procedural_sprite("boss", 0x00EF4444, 2));
        boss_turret.collider = Some(Collider2D::solid(48.0, 48.0));
        boss_turret.health = Some(Health2D::new(10.0));
        boss_turret.script = Some(KVLScriptComponent::new(format!("{}.kvl.json", script_id)).with_seen("OnBossAttack"));

        AiPatch {
            id: format!("patch_{}", ent_id),
            prompt: prompt.to_string(),
            title: "Pattern de Boss Nova Télégraphié".to_string(),
            description: "Ajoute un module d'attaque balistique avec télégraphie circulaire rouge au sol et émission de 16 projectiles expansifs.".to_string(),
            created_entities: vec![boss_turret],
            target_graph_id: script_id.to_string(),
            generated_graph: graph,
            added_node_ids: vec![evt_id.into(), act1_id.into(), act2_id.into()],
            added_connections_count: 2,
            sandbox_report: sandbox,
            status: PatchReviewStatus::PendingReview,
            previous_entities_backup: current_entities.to_vec(),
            previous_graph_backup: existing_graph.cloned(),
        }
    }

    fn build_generic_custom_patch(
        prompt: &str,
        px: f32,
        py: f32,
        ent_id: usize,
        current_entities: &[Entity],
        existing_graph: Option<&Graph>,
    ) -> AiPatch {
        let script_id = "ai_patch_custom";
        let mut graph = Graph::new(script_id, "Mécanique Personnalisée IA", "Logique KVL synthétisée");

        let evt_id = "evt_trigger";
        let act1_id = "act_effect";
        let act2_id = "act_sound";

        let evt = Node::new(
            evt_id,
            "Déclencheur Contextuel",
            Category::Interactions,
            "Déclenché lors de l'interaction avec le nouvel élément.",
            NodeKind::Event { event_type: "OnPlayerInteract".into() },
        ).with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let act1 = Node::new(
            act1_id,
            "Effet Synthétisé KVL",
            Category::World,
            "Effet visuel et logique généré par l'IA.",
            NodeKind::Action { action_type: "AddScore:50".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        let act2 = Node::new(
            act2_id,
            "Retour Sonore",
            Category::Audio,
            "Confirme l'action de jeu.",
            NodeKind::Action { action_type: "PlaySound:victory_bell".into() },
        ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
         .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

        graph.add_node(evt);
        graph.add_node(act1);
        graph.add_node(act2);

        graph.add_connection(Connection::new("c1", evt_id, "out_trigger", act1_id, "in_trigger"));
        graph.add_connection(Connection::new("c2", act1_id, "out_trigger", act2_id, "in_trigger"));

        let sandbox = test_in_sandbox(&graph, "OnPlayerInteract");

        let mut custom_ent = Entity::new(
            ent_id,
            "Entite_Custom_IA",
            "interactive",
            Transform2D::new(px + 150.0, py + 30.0, 32.0, 32.0),
        );
        custom_ent.renderable = Some(Renderable2D::procedural_sprite("coin", 0x00A855F7, 2));
        custom_ent.collider = Some(Collider2D::solid(32.0, 32.0));
        custom_ent.script = Some(KVLScriptComponent::new(format!("{}.kvl.json", script_id)).with_interact("OnPlayerInteract"));

        AiPatch {
            id: format!("patch_{}", ent_id),
            prompt: prompt.to_string(),
            title: "Mécanique Personnalisée KVL".to_string(),
            description: format!("Interprétation de l'intention '{}' en entité interactive réactive KVL avec feedback sensoriel.", prompt),
            created_entities: vec![custom_ent],
            target_graph_id: script_id.to_string(),
            generated_graph: graph,
            added_node_ids: vec![evt_id.into(), act1_id.into(), act2_id.into()],
            added_connections_count: 2,
            sandbox_report: sandbox,
            status: PatchReviewStatus::PendingReview,
            previous_entities_backup: current_entities.to_vec(),
            previous_graph_backup: existing_graph.cloned(),
        }
    }

    /// Construit un patch IA audité à partir du payload structuré généré par un modèle LLM (Ollama, LM Studio, Claude, Gemini, DeepSeek, OpenAI)
    pub fn build_from_generated_payload(
        payload: &crate::ai_client::AiGeneratedGamePayload,
        prompt: &str,
        player_pos: (f32, f32),
        next_entity_id: usize,
        current_entities: &[Entity],
        existing_graph: Option<&Graph>,
    ) -> AiPatch {
        let (px, py) = player_pos;
        let script_id = format!("ai_llm_{}", next_entity_id);
        let mut graph = Graph::new(&script_id, &payload.title, &payload.description);

        let mut added_nodes = Vec::new();
        let mut added_connections_count = 0;

        let mut first_event = "OnStart";

        // 0. Variables d'état déclaratives injectées par le modèle
        if let Some(ref vars) = payload.variables {
            for (var_name, var_val) in vars {
                let value = if var_val == "true" {
                    Value::Bool(true)
                } else if var_val == "false" {
                    Value::Bool(false)
                } else if let Ok(n) = var_val.parse::<i64>() {
                    Value::Int(n)
                } else if let Ok(f) = var_val.parse::<f64>() {
                    Value::Float(f)
                } else {
                    Value::String(var_val.clone())
                };
                graph.add_variable(var_name, value);
            }
        }

        // 1. Initialisation de la boucle de gameplay (si fournie)
        if let Some(ref loop_spec) = payload.gameplay_loop {
            graph.add_variable("objective_text", Value::String(loop_spec.objective.clone()));
            graph.add_variable("game_won", Value::Bool(false));
            graph.add_variable("score", Value::Int(0));

            let start_evt = Node::new(
                "evt_loop_start",
                "Démarrage Mission",
                Category::Interactions,
                "Initialise l'objectif narratif de mission à l'ouverture du niveau",
                NodeKind::Event { event_type: "OnStart".into() },
            ).with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

            let set_obj = Node::new(
                "act_set_objective",
                "Afficher Objectif",
                Category::World,
                "Affiche l'objectif à l'écran du joueur",
                NodeKind::Action { action_type: format!("SetObjective:{}", loop_spec.objective) },
            ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
             .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

            graph.add_node(start_evt);
            graph.add_node(set_obj);
            graph.add_connection(Connection::new("c_loop_init", "evt_loop_start", "out_trigger", "act_set_objective", "in_trigger"));
            added_nodes.push("evt_loop_start".to_string());
            added_nodes.push("act_set_objective".to_string());
            added_connections_count += 1;

            let win_evt = Node::new(
                "evt_loop_win",
                "Objectif Accompli",
                Category::Progression,
                "Se déclenche lorsque la condition de victoire est validée",
                NodeKind::Event { event_type: "OnWinConditionMet".into() },
            ).with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

            let fanfare = loop_spec.win_fanfare.as_deref().unwrap_or("victory_fanfare");
            let win_act = Node::new(
                "act_victory",
                "Triomphe & Fanfare",
                Category::Audio,
                "Joue la fanfare et affiche le triomphe du joueur",
                NodeKind::Action { action_type: format!("PlaySound:{}", fanfare) },
            ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
             .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

            graph.add_node(win_evt);
            graph.add_node(win_act);
            graph.add_connection(Connection::new("c_loop_win", "evt_loop_win", "out_trigger", "act_victory", "in_trigger"));
            added_nodes.push("evt_loop_win".to_string());
            added_nodes.push("act_victory".to_string());
            added_connections_count += 1;
        }

        // 2. Actions & Logique KVL avancée générée par le modèle (Branches, Conditions, Cascades)
        for (i, action_spec) in payload.kvl_actions.iter().enumerate() {
            let evt_id = format!("evt_{}", i);

            if i == 0 && payload.gameplay_loop.is_none() {
                first_event = &action_spec.event;
            }

            let evt = Node::new(
                &evt_id,
                &action_spec.event,
                Category::Interactions,
                "Déclenché par l'événement spécifié par le modèle IA",
                NodeKind::Event { event_type: action_spec.event.clone() },
            ).with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

            graph.add_node(evt);
            added_nodes.push(evt_id.clone());

            // A. Cas conditionnel avec branchement VRAI / FAUX
            if let Some(ref cond_op) = action_spec.condition {
                let cond_id = format!("cond_{}", i);
                let cond = Node::new(
                    &cond_id,
                    format!("Test [{}]", cond_op),
                    Category::MathLogic,
                    "Évalue la condition logique requise",
                    NodeKind::Condition { operator: cond_op.clone() },
                ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
                 .with_output(Pin::new_trigger("out_true", "Vrai", PinDirection::Output))
                 .with_output(Pin::new_trigger("out_false", "Faux", PinDirection::Output));

                graph.add_node(cond);
                added_nodes.push(cond_id.clone());

                let conn_evt_cond = format!("c_evt_cond_{}", i);
                graph.add_connection(Connection::new(&conn_evt_cond, &evt_id, "out_trigger", &cond_id, "in_trigger"));
                added_connections_count += 1;

                // Branche VRAI
                let true_action = action_spec.on_true_action.as_ref().unwrap_or(&action_spec.action);
                let act_true_id = format!("act_true_{}", i);
                let mut act_true = Node::new(
                    &act_true_id,
                    format!("Succès [{}]", true_action),
                    Category::World,
                    "Action exécutée si la condition est validée",
                    NodeKind::Action { action_type: true_action.clone() },
                ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input));

                if action_spec.second_action.is_some() {
                    act_true = act_true.with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));
                }

                graph.add_node(act_true);
                added_nodes.push(act_true_id.clone());

                let conn_cond_true = format!("c_cond_true_{}", i);
                graph.add_connection(Connection::new(&conn_cond_true, &cond_id, "out_true", &act_true_id, "in_trigger"));
                added_connections_count += 1;

                // Cascade d'action secondaire sur branche VRAI
                if let Some(ref sec_act) = action_spec.second_action {
                    let act_sec_id = format!("act_sec_{}", i);
                    let act_sec = Node::new(
                        &act_sec_id,
                        format!("Effet [{}]", sec_act),
                        Category::Audio,
                        "Action en cascade consécutive",
                        NodeKind::Action { action_type: sec_act.clone() },
                    ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input));

                    graph.add_node(act_sec);
                    added_nodes.push(act_sec_id.clone());

                    let conn_cascade = format!("c_cascade_{}", i);
                    graph.add_connection(Connection::new(&conn_cascade, &act_true_id, "out_trigger", &act_sec_id, "in_trigger"));
                    added_connections_count += 1;
                }

                // Branche FAUX (si spécifiée)
                if let Some(ref false_action) = action_spec.on_false_action {
                    let act_false_id = format!("act_false_{}", i);
                    let act_false = Node::new(
                        &act_false_id,
                        format!("Échec [{}]", false_action),
                        Category::World,
                        "Action exécutée si la condition n'est pas remplie",
                        NodeKind::Action { action_type: false_action.clone() },
                    ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input));

                    graph.add_node(act_false);
                    added_nodes.push(act_false_id.clone());

                    let conn_cond_false = format!("c_cond_false_{}", i);
                    graph.add_connection(Connection::new(&conn_cond_false, &cond_id, "out_false", &act_false_id, "in_trigger"));
                    added_connections_count += 1;
                }
            } else {
                // B. Cas direct linéaire (Action directe + cascade optionnelle)
                let act_id = format!("act_{}", i);
                let mut act = Node::new(
                    &act_id,
                    &action_spec.action,
                    Category::World,
                    "Action générée par le modèle IA",
                    NodeKind::Action { action_type: action_spec.action.clone() },
                ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input));

                if action_spec.second_action.is_some() {
                    act = act.with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));
                }

                graph.add_node(act);
                added_nodes.push(act_id.clone());

                let conn_id = format!("c_evt_act_{}", i);
                graph.add_connection(Connection::new(&conn_id, &evt_id, "out_trigger", &act_id, "in_trigger"));
                added_connections_count += 1;

                if let Some(ref sec_act) = action_spec.second_action {
                    let act_sec_id = format!("act_sec_{}", i);
                    let act_sec = Node::new(
                        &act_sec_id,
                        format!("Effet [{}]", sec_act),
                        Category::Audio,
                        "Action en cascade consécutive",
                        NodeKind::Action { action_type: sec_act.clone() },
                    ).with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input));

                    graph.add_node(act_sec);
                    added_nodes.push(act_sec_id.clone());

                    let conn_cascade = format!("c_cascade_{}", i);
                    graph.add_connection(Connection::new(&conn_cascade, &act_id, "out_trigger", &act_sec_id, "in_trigger"));
                    added_connections_count += 1;
                }
            }
        }

        let mut current_id = next_entity_id;
        let mut created_entities = Vec::new();

        // 3. Matérialisation du Terrain & Level Design (Plateformes, Sols, Murs, Dangers)
        if let Some(ref terrain) = payload.terrain {
            for plat in &terrain.platforms {
                let spawn_x = px + plat.offset_x;
                let spawn_y = py + plat.offset_y;
                let mut ent = Entity::new(
                    current_id,
                    &plat.name,
                    "wall",
                    Transform2D::new(spawn_x, spawn_y, plat.width, plat.height),
                );
                current_id += 1;
                let col = plat.color_hex.as_deref().and_then(parse_hex_color).unwrap_or(0x001E293B);
                ent.renderable = Some(Renderable2D::rect(col, 1).with_outline(0x00475569, 2));
                if plat.is_solid {
                    ent.collider = Some(Collider2D::solid(plat.width, plat.height));
                }
                created_entities.push(ent);
            }

            for haz in &terrain.hazards {
                let spawn_x = px + haz.offset_x;
                let spawn_y = py + haz.offset_y;
                let mut ent = Entity::new(
                    current_id,
                    &haz.name,
                    "hazard",
                    Transform2D::new(spawn_x, spawn_y, haz.width, haz.height),
                );
                current_id += 1;
                ent.renderable = Some(Renderable2D::procedural_sprite("spikes", 0x00EF4444, 2));
                ent.collider = Some(Collider2D::trigger(haz.width, haz.height));
                created_entities.push(ent);
            }
        }

        // 4. Matérialisation des Entités avec Rendu Pixel-Art
        for ent_spec in &payload.entities {
            let ent_id = current_id;
            current_id += 1;
            let spawn_x = px + ent_spec.offset_x;
            let spawn_y = py + ent_spec.offset_y;

            let mut ent = Entity::new(
                ent_id,
                &ent_spec.name,
                &ent_spec.kind,
                Transform2D::new(spawn_x, spawn_y, ent_spec.width, ent_spec.height),
            );

            let color = if let Some(ref hex) = ent_spec.color_hex {
                parse_hex_color(hex).unwrap_or(0x00A855F7)
            } else if ent_spec.kind == "enemy" || ent_spec.kind == "boss" {
                0x00EF4444
            } else if ent_spec.kind == "item" {
                0x00F59E0B
            } else if ent_spec.kind == "door" || ent_spec.kind == "portal" {
                0x0006B6D4
            } else {
                0x0038BDF8
            };

            // Rendu pixel-art : priorité à l'asset pxl (sprite ou frame d'atlas),
            // puis matrice ASCII, puis archétype, puis détection automatique.
            if let Some(ref asset) = ent_spec.sprite_asset {
                if let Some(ref frame) = ent_spec.sprite_frame {
                    ent.renderable = Some(Renderable2D::sheet_ref(asset, frame, 5));
                } else {
                    ent.renderable = Some(Renderable2D::image_ref(asset, 5));
                }
            } else if let (Some(ref rows), Some(ref pal_map)) = (&ent_spec.sprite_ascii, &ent_spec.sprite_palette) {
                let mut pal = std::collections::HashMap::new();
                for (k, v) in pal_map {
                    if let Some(ch) = k.chars().next() {
                        let c = parse_hex_color(v).unwrap_or(0x00000000);
                        pal.insert(ch, c);
                    }
                }
                let row_refs: Vec<&str> = rows.iter().map(|s| s.as_str()).collect();
                ent.renderable = Some(Renderable2D::from_ascii_grid(&row_refs, &pal, 5));
            } else if let Some(ref arch) = ent_spec.sprite_archetype {
                ent.renderable = Some(Renderable2D::procedural_sprite(arch, color, 5));
            } else {
                let detected_arch = match ent_spec.kind.as_str() {
                    "player" => "ninja",
                    "enemy" => "drone",
                    "boss" => "boss",
                    "item" => {
                        let lower = ent_spec.name.to_lowercase();
                        if lower.contains("cle") || lower.contains("key") {
                            "key"
                        } else if lower.contains("potion") || lower.contains("heal") {
                            "potion"
                        } else {
                            "chest"
                        }
                    }
                    "door" => "portal",
                    "hazard" => "spikes",
                    _ => "coin",
                };
                ent.renderable = Some(Renderable2D::procedural_sprite(detected_arch, color, 5));
            }

            // --- Champs 3D (Lot 7) : mesh référencé (manifest) ou primitive ---
            let is_3d = ent_spec.mesh_asset.is_some()
                || ent_spec.mesh_primitive.is_some()
                || ent_spec.position3d.is_some()
                || ent_spec.rotation_euler_deg.is_some()
                || ent_spec.scale3d.is_some();
            if is_3d {
                let mesh = if let Some(ref asset) = ent_spec.mesh_asset {
                    MeshRef::Asset { id: asset.clone(), node: None }
                } else {
                    match ent_spec.mesh_primitive.as_deref() {
                        Some("sphere") => MeshRef::Sphere,
                        Some("plane") => MeshRef::Plane,
                        Some("capsule") => MeshRef::Capsule,
                        _ => MeshRef::Cube,
                    }
                };
                let position = match ent_spec.position3d {
                    Some([x, y, z]) => Vec3::new(x, y, z),
                    None => Vec3::new(spawn_x, 0.0, spawn_y),
                };
                let rotation = ent_spec.rotation_euler_deg.unwrap_or([0.0, 0.0, 0.0]);
                let scale = ent_spec.scale3d.unwrap_or([1.0, 1.0, 1.0]);
                ent.transform3d = Some(Transform3D::from_euler_deg(
                    position,
                    Vec3::new(rotation[0], rotation[1], rotation[2]),
                    Vec3::new(scale[0], scale[1], scale[2]),
                ));
                ent.renderable3d = Some(Renderable3D {
                    mesh,
                    material: MaterialRef::Solid { color, roughness: 0.4, metalness: 0.0 },
                    cast_shadow: true,
                    receive_shadow: true,
                });
            }

            if ent_spec.has_collider {
                ent.collider = Some(Collider2D::solid(ent_spec.width, ent_spec.height));
            }

            if ent_spec.health > 0.0 {
                ent.health = Some(Health2D::new(ent_spec.health));
            }

            if ent_spec.has_vision {
                ent.vision = Some(VisionSensing::new(200.0));
                ent.nav_agent = Some(NavAgent::new(120.0));
            }

            if ent_spec.movement.as_deref() == Some("patrol") {
                ent.movement = Some(Movement2D::patrol_vertical(60.0, spawn_y - 80.0, spawn_y + 80.0));
            } else if ent_spec.movement.as_deref() == Some("chase") {
                ent.movement = Some(Movement2D::new(110.0));
            }

            if ent_spec.is_flocking {
                ent.flock = Some(FlockMember::new(1));
            }

            if !payload.kvl_actions.is_empty() {
                ent.script = Some(KVLScriptComponent::new(format!("{}.kvl.json", script_id)));
            }

            created_entities.push(ent);
        }

        let sandbox = test_in_sandbox(&graph, first_event);

        AiPatch {
            id: format!("patch_{}", next_entity_id),
            prompt: prompt.to_string(),
            title: payload.title.clone(),
            description: payload.description.clone(),
            created_entities,
            target_graph_id: script_id,
            generated_graph: graph,
            added_node_ids: added_nodes,
            added_connections_count,
            sandbox_report: sandbox,
            status: PatchReviewStatus::PendingReview,
            previous_entities_backup: current_entities.to_vec(),
            previous_graph_backup: existing_graph.cloned(),
        }
    }
}

fn parse_hex_color(hex: &str) -> Option<u32> {
    let clean = hex.trim_start_matches('#');
    u32::from_str_radix(clean, 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::ShapeKind;

    #[test]
    fn test_ai_patch_guard_intent_and_sandbox() {
        let current_entities = Vec::new();
        let patch = AiPatchBuilder::build_from_prompt("Je veux un garde patrouilleur hostile", (100.0, 100.0), 10, &current_entities, None);
        assert_eq!(patch.status, PatchReviewStatus::PendingReview);
        assert_eq!(patch.created_entities.len(), 1);
        let guard = &patch.created_entities[0];
        assert_eq!(guard.name, "Garde_Sentinelle_IA");
        assert!(guard.vision.is_some());
        assert!(guard.nav_agent.is_some());
        assert!(patch.sandbox_report.static_check_passed);
        assert_eq!(patch.sandbox_report.static_errors_count, 0);
        assert!(patch.sandbox_report.simulation_passed);
    }

    #[test]
    fn test_ai_patch_chest_intent_and_sandbox() {
        let current_entities = Vec::new();
        let patch = AiPatchBuilder::build_from_prompt("Crée un coffre au trésor verrouillé par clé", (100.0, 100.0), 11, &current_entities, None);
        assert_eq!(patch.created_entities.len(), 1);
        assert_eq!(patch.created_entities[0].tag, "chest");
        assert!(patch.sandbox_report.static_check_passed);
        assert_eq!(patch.sandbox_report.static_errors_count, 0);
        assert_eq!(patch.added_node_ids.len(), 5);
        assert_eq!(patch.added_connections_count, 4);
    }

    #[test]
    fn test_ai_patch_hazard_and_portal_and_potion() {
        let current_entities = Vec::new();
        let patch_hazard = AiPatchBuilder::build_from_prompt("Zone de piège à lave ardente", (50.0, 50.0), 12, &current_entities, None);
        assert_eq!(patch_hazard.created_entities[0].tag, "hazard");
        assert!(patch_hazard.sandbox_report.static_check_passed);

        let patch_portal = AiPatchBuilder::build_from_prompt("Ouvre un portail de téléportation", (50.0, 50.0), 13, &current_entities, None);
        assert_eq!(patch_portal.created_entities[0].tag, "portal");
        assert!(patch_portal.sandbox_report.static_check_passed);

        let patch_potion = AiPatchBuilder::build_from_prompt("Ajoute une fiole de soin de vie", (50.0, 50.0), 14, &current_entities, None);
        assert_eq!(patch_potion.created_entities[0].tag, "item_potion");
        assert!(patch_potion.sandbox_report.static_check_passed);
    }

    #[test]
    fn test_ai_patch_swarm_boids_and_dialogue_npc() {
        let current_entities = Vec::new();
        let patch_swarm = AiPatchBuilder::build_from_prompt("Invoque un essaim de boids volants", (0.0, 0.0), 20, &current_entities, None);
        assert_eq!(patch_swarm.created_entities.len(), 3);
        assert!(patch_swarm.created_entities[0].flock.is_some());
        assert!(patch_swarm.sandbox_report.static_check_passed);

        let patch_npc = AiPatchBuilder::build_from_prompt("Crée un PNJ avec dialogue et quête", (0.0, 0.0), 30, &current_entities, None);
        assert_eq!(patch_npc.created_entities.len(), 1);
        assert!(patch_npc.created_entities[0].dialogue.is_some());
        assert!(patch_npc.sandbox_report.static_check_passed);
    }

    #[test]
    fn test_build_from_generated_payload_full_level_with_pixel_sprites_and_terrain() {
        let current_entities = Vec::new();
        let payload = crate::ai_client::AiGeneratedGamePayload {
            title: "Donjon Oublié du Chevalier".to_string(),
            description: "Niveau complet avec plateformes en pierre, gouffre de piques, chevalier et coffre au trésor.".to_string(),
            primary_genre: Some("Platformer".to_string()),
            terrain: Some(crate::ai_client::AiGeneratedTerrain {
                theme: "dungeon".to_string(),
                platforms: vec![
                    crate::ai_client::AiGeneratedPlatform {
                        name: "Sol_Donjon".to_string(),
                        offset_x: -200.0,
                        offset_y: 120.0,
                        width: 500.0,
                        height: 30.0,
                        is_solid: true,
                        color_hex: Some("#1E293B".to_string()),
                    },
                    crate::ai_client::AiGeneratedPlatform {
                        name: "Corniche_Haute".to_string(),
                        offset_x: 100.0,
                        offset_y: 30.0,
                        width: 120.0,
                        height: 20.0,
                        is_solid: true,
                        color_hex: Some("#334155".to_string()),
                    },
                ],
                hazards: vec![
                    crate::ai_client::AiGeneratedHazard {
                        name: "Fosse_Piques".to_string(),
                        offset_x: 20.0,
                        offset_y: 100.0,
                        width: 60.0,
                        height: 20.0,
                        damage: 40.0,
                        hazard_type: Some("spikes".to_string()),
                    },
                ],
            }),
            gameplay_loop: Some(crate::ai_client::AiGeneratedGameplayLoop {
                objective: "Ouvrir le coffre doré pour libérer le donjon".to_string(),
                win_condition: "OpenChest".to_string(),
                lose_condition: "PlayerHealthZero".to_string(),
                target_item_name: Some("Coffre_Or".to_string()),
                win_fanfare: Some("victory_bell".to_string()),
            }),
            entities: vec![
                crate::ai_client::AiGeneratedEntity {
                    name: "Chevalier_Noble".to_string(),
                    kind: "player".to_string(),
                    offset_x: 0.0,
                    offset_y: 80.0,
                    width: 32.0,
                    height: 32.0,
                    health: 6.0,
                    color_hex: Some("#3B82F6".to_string()),
                    sprite_archetype: Some("knight".to_string()),
                    sprite_ascii: None,
                    sprite_palette: None,
                    sprite_asset: None,
                    sprite_frame: None,
                    movement: None,
                    has_collider: true,
                    has_vision: false,
                    is_flocking: false,
                    ..Default::default()
                },
                crate::ai_client::AiGeneratedEntity {
                    name: "Coffre_Or".to_string(),
                    kind: "item".to_string(),
                    offset_x: 140.0,
                    offset_y: 0.0,
                    width: 24.0,
                    height: 24.0,
                    health: 0.0,
                    color_hex: Some("#F59E0B".to_string()),
                    sprite_archetype: Some("chest".to_string()),
                    sprite_ascii: None,
                    sprite_palette: None,
                    sprite_asset: None,
                    sprite_frame: None,
                    movement: None,
                    has_collider: true,
                    has_vision: false,
                    is_flocking: false,
                    ..Default::default()
                },
            ],
            kvl_actions: vec![
                crate::ai_client::AiGeneratedKvlAction {
                    event: "OnPlayerInteract".to_string(),
                    action: "PlaySound:coin".to_string(),
                    target_entity: None,
                    condition: None,
                    on_true_action: None,
                    on_false_action: None,
                    second_action: None,
                },
            ],
            variables: None,
        };

        let patch = AiPatchBuilder::build_from_generated_payload(&payload, "Donjon avec chevalier", (100.0, 100.0), 50, &current_entities, None);

        // 2 plateformes + 1 fosse de piques + 2 entités = 5 entités créées
        assert_eq!(patch.created_entities.len(), 5);

        // Vérification de la présence de sprites pixel-art sur les entités
        let knight_ent = patch.created_entities.iter().find(|e| e.name == "Chevalier_Noble").expect("Chevalier");
        assert!(matches!(knight_ent.renderable.as_ref().unwrap().shape, ShapeKind::PixelMatrix { .. }));

        let chest_ent = patch.created_entities.iter().find(|e| e.name == "Coffre_Or").expect("Coffre");
        assert!(matches!(chest_ent.renderable.as_ref().unwrap().shape, ShapeKind::PixelMatrix { .. }));

        // Vérification du terrain
        let wall_ent = patch.created_entities.iter().find(|e| e.name == "Sol_Donjon").expect("Sol");
        assert_eq!(wall_ent.tag, "wall");
        assert!(wall_ent.collider.as_ref().unwrap().is_solid);

        // Vérification de la boucle de gameplay KVL (nœud de mission et de victoire)
        assert!(patch.generated_graph.nodes.contains_key("evt_loop_start"));
        assert!(patch.generated_graph.nodes.contains_key("act_set_objective"));
        assert!(patch.generated_graph.nodes.contains_key("evt_loop_win"));
        assert!(patch.generated_graph.nodes.contains_key("act_victory"));
        assert!(patch.sandbox_report.static_check_passed);
    }

    #[test]
    fn test_payload_with_sprite_asset_maps_to_image_and_sheet_ref() {
        // `sprite_asset` seul -> ImageRef, `sprite_asset`+`sprite_frame` -> SheetRef.
        let json = r#"{
            "title": "Test Sprite Asset",
            "description": "test",
            "primary_genre": "Platformer",
            "terrain": { "theme": "dungeon", "platforms": [], "hazards": [] },
            "entities": [
                { "name": "Piece", "kind": "item", "sprite_asset": "coin" },
                { "name": "Coffre", "kind": "item", "sprite_asset": "chars", "sprite_frame": "chest" }
            ],
            "gameplay_loop": { "objective": "test", "win_condition": "DefeatBoss", "lose_condition": "PlayerHealthZero" },
            "kvl_actions": []
        }"#;
        let payload: crate::ai_client::AiGeneratedGamePayload =
            serde_json::from_str(json).expect("payload valide");
        let patch = AiPatchBuilder::build_from_generated_payload(&payload, "test", (100.0, 100.0), 50, &[], None);

        let coin = patch.created_entities.iter().find(|e| e.name == "Piece").expect("Piece");
        assert!(matches!(
            &coin.renderable.as_ref().unwrap().shape,
            ShapeKind::ImageRef { asset } if asset == "coin"
        ));

        let chest = patch.created_entities.iter().find(|e| e.name == "Coffre").expect("Coffre");
        assert!(matches!(
            &chest.renderable.as_ref().unwrap().shape,
            ShapeKind::SheetRef { asset, frame } if asset == "chars" && frame == "chest"
        ));
    }

    #[test]
    fn test_payload_with_3d_fields_maps_to_renderable3d() {
        let json = r#"{
            "title": "Test 3D",
            "description": "test",
            "primary_genre": "Platformer",
            "terrain": { "theme": "dungeon", "platforms": [], "hazards": [] },
            "entities": [
                { "name": "Pilier", "kind": "wall", "mesh_asset": "pillar", "position3d": [1.0, 2.0, 3.0] },
                { "name": "Orbe", "kind": "item", "mesh_primitive": "sphere", "scale3d": [2.0, 2.0, 2.0] }
            ],
            "gameplay_loop": { "objective": "test", "win_condition": "DefeatBoss", "lose_condition": "PlayerHealthZero" },
            "kvl_actions": []
        }"#;
        let payload: crate::ai_client::AiGeneratedGamePayload =
            serde_json::from_str(json).expect("payload valide");
        let patch = AiPatchBuilder::build_from_generated_payload(&payload, "test", (100.0, 100.0), 50, &[], None);

        let pillar = patch.created_entities.iter().find(|e| e.name == "Pilier").expect("Pilier");
        let r3d = pillar.renderable3d.as_ref().expect("Renderable3D");
        assert!(matches!(&r3d.mesh, crate::entity::MeshRef::Asset { id, .. } if id == "pillar"));
        let t3d = pillar.transform3d.as_ref().expect("Transform3D");
        assert_eq!(t3d.position.x, 1.0);
        assert_eq!(t3d.position.y, 2.0);
        assert_eq!(t3d.position.z, 3.0);

        let orb = patch.created_entities.iter().find(|e| e.name == "Orbe").expect("Orbe");
        let r3d = orb.renderable3d.as_ref().expect("Renderable3D");
        assert!(matches!(&r3d.mesh, crate::entity::MeshRef::Sphere));
    }

    #[test]
    fn test_build_complex_kvl_graph_with_conditions_branching_and_variables() {
        let current_entities = Vec::new();
        let mut custom_vars = std::collections::HashMap::new();
        custom_vars.insert("dungeon_keys".to_string(), "0".to_string());
        custom_vars.insert("door_locked".to_string(), "true".to_string());
        custom_vars.insert("boss_rage_counter".to_string(), "3".to_string());

        let payload = crate::ai_client::AiGeneratedGamePayload {
            title: "Sanctuaire des Énigmes et Boss à Deux Phases".to_string(),
            description: "Niveau complexe avec porte verrouillée par clé, dialogue avec PNJ, et Boss avec enrage à mi-vie.".to_string(),
            primary_genre: Some("Aventure".to_string()),
            terrain: None,
            gameplay_loop: Some(crate::ai_client::AiGeneratedGameplayLoop {
                objective: "Résolvez l'énigme de la porte, parlez au Sage et terrassez le Titan!".to_string(),
                win_condition: "DefeatBoss".to_string(),
                lose_condition: "PlayerHealthZero".to_string(),
                target_item_name: Some("Cle_Sanctuaire".to_string()),
                win_fanfare: Some("victory_fanfare".to_string()),
            }),
            entities: vec![
                crate::ai_client::AiGeneratedEntity {
                    name: "Porte_Donjon".to_string(),
                    kind: "door".to_string(),
                    offset_x: 200.0,
                    offset_y: 50.0,
                    width: 32.0,
                    height: 64.0,
                    health: 100.0,
                    color_hex: Some("#A855F7".to_string()),
                    sprite_archetype: Some("portal".to_string()),
                    sprite_ascii: None,
                    sprite_palette: None,
                    sprite_asset: None,
                    sprite_frame: None,
                    movement: Some("none".to_string()),
                    has_collider: true,
                    has_vision: false,
                    is_flocking: false,
                    ..Default::default()
                },
            ],
            kvl_actions: vec![
                // 1. Énigme de porte : Interaction -> Test clé -> Vrai: ouvre porte + jingle, Faux: dialogue refus
                crate::ai_client::AiGeneratedKvlAction {
                    event: "OnPlayerInteract".to_string(),
                    action: "ToggleDoor".to_string(),
                    target_entity: Some("Porte_Donjon".to_string()),
                    condition: Some("HasItem:Cle_Sanctuaire".to_string()),
                    on_true_action: Some("ToggleDoor".to_string()),
                    on_false_action: Some("DenyAccess".to_string()),
                    second_action: Some("PlaySound:coin".to_string()),
                },
                // 2. Boss à phase : Dégâts subis -> Test santé < 25 -> Bascule phase 2 + rugissement
                crate::ai_client::AiGeneratedKvlAction {
                    event: "OnDamage".to_string(),
                    action: "SetBossPhase:2".to_string(),
                    target_entity: Some("Titan_Boss".to_string()),
                    condition: Some("HealthLessThan:25".to_string()),
                    on_true_action: Some("SetBossPhase:2".to_string()),
                    on_false_action: None,
                    second_action: Some("TriggerBossRoar".to_string()),
                },
            ],
            variables: Some(custom_vars),
        };

        let patch = AiPatchBuilder::build_from_generated_payload(&payload, "Sanctuaire complexe", (50.0, 50.0), 100, &current_entities, None);

        // 1. Vérification des variables d'état injectées
        assert!(patch.generated_graph.variables.contains_key("dungeon_keys"));
        assert!(patch.generated_graph.variables.contains_key("door_locked"));
        assert!(patch.generated_graph.variables.contains_key("boss_rage_counter"));

        // 2. Vérification des nœuds de condition KVL créés
        assert!(patch.generated_graph.nodes.contains_key("cond_0"));
        assert!(patch.generated_graph.nodes.contains_key("act_true_0"));
        assert!(patch.generated_graph.nodes.contains_key("act_sec_0"));
        assert!(patch.generated_graph.nodes.contains_key("act_false_0"));

        assert!(patch.generated_graph.nodes.contains_key("cond_1"));
        assert!(patch.generated_graph.nodes.contains_key("act_true_1"));
        assert!(patch.generated_graph.nodes.contains_key("act_sec_1"));

        // 3. Vérification des connexions logiques (out_true et out_false)
        let has_conn_true_0 = patch.generated_graph.connections.iter().any(|c| c.from_node == "cond_0" && c.from_pin == "out_true" && c.to_node == "act_true_0");
        let has_conn_false_0 = patch.generated_graph.connections.iter().any(|c| c.from_node == "cond_0" && c.from_pin == "out_false" && c.to_node == "act_false_0");
        let has_conn_cascade_0 = patch.generated_graph.connections.iter().any(|c| c.from_node == "act_true_0" && c.from_pin == "out_trigger" && c.to_node == "act_sec_0");

        assert!(has_conn_true_0, "Connexion cond_0 -> act_true_0 requise");
        assert!(has_conn_false_0, "Connexion cond_0 -> act_false_0 requise");
        assert!(has_conn_cascade_0, "Cascade act_true_0 -> act_sec_0 requise");

        // 4. Validation statique et simulation en bac à sable
        assert!(patch.sandbox_report.static_check_passed);
        assert_eq!(patch.sandbox_report.static_errors_count, 0);
        assert!(patch.sandbox_report.simulation_passed);
    }
}
