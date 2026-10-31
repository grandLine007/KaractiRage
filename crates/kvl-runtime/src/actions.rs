//! Nœuds cœur du moteur, migrés vers le registre de handlers.
//!
//! Ce module est le **node pack de référence** (`karacti.nodes.core`) : il illustre le
//! contrat attendu d'un pack de nœuds compilé — des schémas ([`NodeSchema`]) et des
//! handlers purs, sans dépendance au moteur de jeu.
//!
//! Les trois actions migrées (`AddScore`, `DamagePlayer`, `GiveItem`) conservent
//! exactement les variables, les messages de trace et les paramètres préfixés du
//! `match` historique. Les autres actions restent exécutées par le repli de
//! l'interpréteur jusqu'à leur migration.

use kvl_core::{Category, NodeSchema, ParamSpec, Value};

use crate::context::ExecutionContext;
use crate::handlers::{HandlerRegistry, NodeCall};
use crate::interpreter::RuntimeError;

/// Identifiant du pack de nœuds cœur, référencé par les manifestes.
pub const CORE_NODE_PACK: &str = "karacti.nodes.core";

/// Schémas exposés par le pack cœur.
pub fn builtin_node_schemas() -> Vec<NodeSchema> {
    let mut schemas = vec![
        NodeSchema::action(
            "AddScore",
            "Ajouter 1 au score",
            Category::Progression,
            "Incrémente la variable « score » de 1. Nœud de base des collectibles.",
        )
        .with_param(ParamSpec::int("amount", "Points", 1))
        .with_prefix("AddScore:")
        .with_plugin(CORE_NODE_PACK),
        NodeSchema::action(
            "DamagePlayer",
            "Infliger des dégâts",
            Category::Combat,
            "Retire des points de vie au joueur. Sans argument : 1 PV. \
             Forme paramétrée : « DamagePlayer:3 » retire 3 PV.",
        )
        .with_param(ParamSpec::int("amount", "Dégâts", 1).with_description(
            "Nombre de PV retirés (forme préfixée « DamagePlayer:N »).",
        ))
        .with_prefix("DamagePlayer:")
        .with_plugin(CORE_NODE_PACK),
        NodeSchema::action(
            "GiveItem",
            "Donner un objet",
            Category::World,
            "Ajoute un objet à l'inventaire. Forme : « GiveItem:identifiant:quantité ».",
        )
        .with_param(
            ParamSpec::text("item", "Identifiant de l'objet", "coin")
                .with_description("Clé d'inventaire (ex. « coin », « potion_heal »)."),
        )
        .with_param(
            ParamSpec::int("count", "Quantité", 1)
                .with_description("Nombre d'exemplaires ajoutés."),
        )
        .with_prefix("GiveItem:")
        .with_plugin(CORE_NODE_PACK),
        NodeSchema::action(
            "PlaySound",
            "Jouer un son",
            Category::Audio,
            "Demande un effet sonore au moteur. Forme : « PlaySound:coin »              (coin, alarm, impact, potion_drink, teleport, victory_bell…).",
        )
        .with_param(ParamSpec::text("sound", "Identifiant du son", "coin"))
        .with_prefix("PlaySound:")
        .with_plugin(CORE_NODE_PACK),
        NodeSchema::action(
            "OpenDoor",
            "Ouvrir une porte",
            Category::World,
            "Ouvre la porte du graphe (variable « door_is_open »).",
        )
        .with_plugin(CORE_NODE_PACK),
        NodeSchema::action(
            "SetVariable",
            "Définir une variable",
            Category::MathLogic,
            "Fixe une variable typée. Forme : « SetVariable:nom:valeur »              (true/false, entier, décimal ou texte).",
        )
        .with_param(ParamSpec::text("name", "Nom de la variable", "flag"))
        .with_param(ParamSpec::text("value", "Valeur", "true"))
        .with_prefix("SetVariable:")
        .with_plugin(CORE_NODE_PACK),
    ];
    schemas.extend(crate::actions_batch::schemas());
    schemas.extend(core_event_schemas());
    schemas
}

/// Événements déclenchés par le moteur (`trigger_event`), décrits pour que le
/// catalogue et l'éditeur puissent les proposer et les valider.
///
/// Aucun handler n'est nécessaire : le déclenchement reste dans la boucle du moteur.
pub fn core_event_schemas() -> Vec<NodeSchema> {
    fn event(id: &str, category: Category, description: &str) -> NodeSchema {
        let mut schema = NodeSchema::event(id, id, category, description);
        schema.plugin = Some(CORE_NODE_PACK.to_string());
        schema
    }

    vec![
        event("Start", Category::World, "Déclenché au démarrage de la scène."),
        event("OnStart", Category::World, "Déclenché une fois au lancement du niveau."),
        event("OnPlayerOverlap", Category::Interactions, "Contact entre le joueur et une entité."),
        event("OnPlayerInteract", Category::Interactions, "Interaction volontaire du joueur (touche d'action)."),
        event("OnDoorInteract", Category::Interactions, "Le joueur interagit avec une porte."),
        event("OnItemPickup", Category::Interactions, "Le joueur ramasse un objet."),
        event("OnItemUse", Category::Interactions, "Le joueur utilise un objet de l'inventaire."),
        event("OnChestOpened", Category::Interactions, "Un coffre est ouvert."),
        event("OnKeyFound", Category::Interactions, "Une clé est trouvée."),
        event("OnPortal", Category::Interactions, "Le joueur traverse un portail."),
        event("OnPressurePlate", Category::Interactions, "Une plaque de pression est activée."),
        event("OnTalk", Category::Interactions, "Un dialogue est engagé avec un personnage."),
        event("OnUse", Category::Interactions, "Action générique d'utilisation."),
        event("OnPlayerSeen", Category::ArtificialIntelligence, "Un ennemi aperçoit le joueur."),
        event("OnVisionAlert", Category::ArtificialIntelligence, "Alerte visuelle d'une sentinelle."),
        event("OnAlert", Category::ArtificialIntelligence, "Alerte générale."),
        event("OnSwarmAlert", Category::ArtificialIntelligence, "Alerte de meute."),
        event("OnIdle", Category::ArtificialIntelligence, "Retour à l'état de veille."),
        event("OnDialogueStart", Category::UserInterface, "Ouverture d'une boîte de dialogue."),
        event("OnDialogueChoice", Category::UserInterface, "Choix dans un dialogue."),
        event("OnDialogueClose", Category::UserInterface, "Fermeture d'un dialogue."),
        event("OnBossAttack", Category::Combat, "Attaque télégraphiée d'un boss."),
        event("OnBossDefeated", Category::Combat, "Le boss est vaincu."),
        event("OnCheck", Category::MathLogic, "Point de contrôle logique (tests unitaires)."),
        event("OnWinConditionMet", Category::Progression, "La condition de victoire est remplie."),
    ]
}

/// Handlers des nœuds cœur.
pub fn builtin_handlers() -> HandlerRegistry {
    let mut registry = HandlerRegistry::new();
    registry
        .register("AddScore", add_score)
        .expect("handler AddScore unique");
    registry
        .register_prefix("AddScore:", add_score)
        .expect("préfixe AddScore unique");
    registry
        .register("DamagePlayer", damage_player)
        .expect("handler DamagePlayer unique");
    registry
        .register_prefix("DamagePlayer:", damage_player)
        .expect("préfixe DamagePlayer unique");
    registry
        .register_prefix("GiveItem:", give_item)
        .expect("préfixe GiveItem unique");
    registry
        .register("PlaySound", play_sound)
        .expect("handler PlaySound unique");
    registry
        .register_prefix("PlaySound:", play_sound)
        .expect("préfixe PlaySound unique");
    registry
        .register_prefix("TeleportPlayer:", teleport_player)
        .expect("préfixe TeleportPlayer unique");
    registry
        .register("OpenDoor", open_door)
        .expect("handler OpenDoor unique");
    registry
        .register_prefix("SetVariable:", set_variable)
        .expect("préfixe SetVariable unique");

    // Actions historiques extraites du dispatch de l'interpréteur.
    crate::actions_batch::register(&mut registry);

    registry
}

/// Demande un son au moteur via `sound_request`, consommé en fin de frame.
///
/// Sans argument (`PlaySound`), la demande est vide : aucun son n'est joué, ce qui
/// préserve le comportement historique des graphes guidés.
fn play_sound(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    if call.argument.is_empty() {
        ctx.record_step(
            call.node_id,
            call.node_name,
            "PlaySound sans identifiant : aucun son demandé.",
        );
        return Ok(());
    }

    ctx.set_var("sound_request", Value::String(call.argument.to_string()));
    ctx.record_step(
        call.node_id,
        call.node_name,
        format!("Son demandé : {}", call.argument),
    );
    Ok(())
}

/// Forme `TeleportPlayer:x:y` produite par les recettes guidées (et `x,y` toléré).
///
/// Le dispatch historique ne connaissait que `TeleportTo:x,y` et l'action exacte
/// `TeleportPlayer` : les portails générés par l'assistant étaient donc muets.
fn teleport_player(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let mut parts = call.argument.split([':', ',']);
    let tx = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(120);
    let ty = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(480);
    ctx.set_var("teleport_target_x", Value::Int(tx));
    ctx.set_var("teleport_target_y", Value::Int(ty));
    ctx.record_step(
        call.node_id,
        call.node_name,
        format!("Téléportation de l'entité vers ({}, {}) !", tx, ty),
    );
    Ok(())
}

fn open_door(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    ctx.set_var("door_is_open", Value::Bool(true));
    ctx.record_step(call.node_id, call.node_name, "Porte ouverte !");
    Ok(())
}

fn set_variable(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let mut parts = call.argument.splitn(2, ':');
    let name = parts.next().unwrap_or("");
    let raw = parts.next().unwrap_or("");
    if name.is_empty() {
        ctx.record_step(
            call.node_id,
            call.node_name,
            "SetVariable sans nom : ignoré.",
        );
        return Ok(());
    }

    let value = match raw {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => {
            if let Ok(int) = raw.parse::<i64>() {
                Value::Int(int)
            } else if let Ok(float) = raw.parse::<f64>() {
                Value::Float(float)
            } else {
                Value::String(raw.to_string())
            }
        }
    };

    ctx.set_var(name, value);
    ctx.record_step(
        call.node_id,
        call.node_name,
        format!("Variable '{}' définie à '{}'", name, raw),
    );
    Ok(())
}

fn add_score(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let amount = if call.argument.is_empty() {
        1
    } else {
        call.argument.parse::<i64>().unwrap_or(1)
    };
    let current_score = match ctx.get_var("score") {
        Some(Value::Int(s)) => *s,
        _ => 0,
    };
    ctx.set_var("score", Value::Int(current_score + amount));

    if !call.argument.is_empty() {
        ctx.record_step(
            call.node_id,
            call.node_name,
            format!("Score +{} (Total: {})", amount, current_score + amount),
        );
    }
    Ok(())
}

fn damage_player(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let amount = if call.argument.is_empty() {
        1
    } else {
        call.argument.parse::<i64>().unwrap_or(1)
    };

    let current_hp = match ctx.get_var("player_hp") {
        Some(Value::Int(hp)) => *hp,
        _ => 3,
    };
    let new_hp = (current_hp - amount).max(0);
    ctx.set_var("player_hp", Value::Int(new_hp));

    if call.argument.is_empty() {
        ctx.record_step(
            call.node_id,
            call.node_name,
            format!("Dégâts infligés au joueur : -1 PV (Reste: {})", new_hp),
        );
    } else {
        ctx.record_step(
            call.node_id,
            call.node_name,
            format!(
                "Dégâts infligés au joueur : -{} PV (Reste: {})",
                amount, new_hp
            ),
        );
    }

    Ok(())
}

fn give_item(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let mut parts = call.argument.split(':');
    let item_id = parts.next().unwrap_or("item");
    let count = parts
        .next()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(1);

    let current_count = match ctx.get_var(&format!("item_count_{}", item_id)) {
        Some(Value::Int(c)) => *c,
        _ => 0,
    };
    let new_count = current_count + count;

    ctx.set_var(format!("item_count_{}", item_id), Value::Int(new_count));
    ctx.set_var(format!("has_{}", item_id), Value::Bool(true));
    ctx.set_var(
        "last_picked_item",
        Value::String(item_id.to_string()),
    );
    ctx.record_step(
        call.node_id,
        call.node_name,
        format!(
            "Objet '{}' ajouté à l'inventaire (+{}) (Total: {})",
            item_id, count, new_count
        ),
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> HandlerRegistry {
        builtin_handlers()
    }

    #[test]
    fn add_score_increments_by_one() {
        let mut ctx = ExecutionContext::new();
        ctx.set_var("score", Value::Int(41));
        let call = NodeCall::new("n", "N", "AddScore", "");
        let outcome = registry()
            .dispatch(&mut ctx, call.node_id, call.node_name, call.type_id)
            .expect("handler");
        outcome.expect("exécution");
        assert_eq!(ctx.get_var("score"), Some(&Value::Int(42)));
    }

    #[test]
    fn damage_player_exact_and_prefixed() {
        let mut ctx = ExecutionContext::new();
        ctx.set_var("player_hp", Value::Int(5));

        registry().dispatch(&mut ctx, "n1", "N1", "DamagePlayer");
        assert_eq!(ctx.get_var("player_hp"), Some(&Value::Int(4)));

        registry().dispatch(&mut ctx, "n2", "N2", "DamagePlayer:2");
        assert_eq!(ctx.get_var("player_hp"), Some(&Value::Int(2)));

        registry().dispatch(&mut ctx, "n3", "N3", "DamagePlayer:99");
        assert_eq!(ctx.get_var("player_hp"), Some(&Value::Int(0)));
    }

    #[test]
    fn give_item_keeps_flag_and_counter_in_sync() {
        let mut ctx = ExecutionContext::new();
        registry().dispatch(&mut ctx, "n1", "N1", "GiveItem:potion_heal:2");
        assert_eq!(
            ctx.get_var("item_count_potion_heal"),
            Some(&Value::Int(2))
        );
        assert_eq!(
            ctx.get_var("has_potion_heal"),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            ctx.get_var("last_picked_item"),
            Some(&Value::String("potion_heal".into()))
        );

        registry().dispatch(&mut ctx, "n2", "N2", "GiveItem:potion_heal");
        assert_eq!(
            ctx.get_var("item_count_potion_heal"),
            Some(&Value::Int(3))
        );
    }

    #[test]
    fn all_builtin_schemas_are_valid_and_actions_have_handlers() {
        let schemas = builtin_node_schemas();
        let registry = registry();
        let actions: Vec<&NodeSchema> = schemas
            .iter()
            .filter(|schema| schema.flavor == kvl_core::NodeFlavor::Action)
            .collect();

        for id in ["AddScore", "DamagePlayer", "GiveItem"] {
            assert!(
                schemas.iter().any(|schema| schema.id == id
                    && schema.flavor == kvl_core::NodeFlavor::Action),
                "action cœur absente du catalogue : {}",
                id
            );
        }
        assert!(actions.len() >= 40, "catalogue d'actions incomplet");
        assert!(schemas.len() > actions.len(), "les événements doivent être catalogués");

        for schema in &schemas {
            schema.validate().expect("schéma valide");
            if schema.flavor == kvl_core::NodeFlavor::Action {
                let handled = registry.is_registered(&schema.id)
                    || registry.is_registered(&format!("{}:1", schema.id))
                    || schema
                        .prefixes
                        .iter()
                        .any(|prefix| registry.is_registered(&format!("{}1", prefix)));
                assert!(handled, "handler manquant pour {}", schema.id);
            } else {
                assert_eq!(schema.flavor, kvl_core::NodeFlavor::Event);
            }
        }
    }

    #[test]
    fn unknown_action_is_not_handled() {
        let mut ctx = ExecutionContext::new();
        assert!(registry()
            .dispatch(&mut ctx, "n", "N", "NotMigratedYet")
            .is_none());
    }
}
