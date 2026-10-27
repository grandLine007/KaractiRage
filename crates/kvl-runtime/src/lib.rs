pub mod actions;
mod actions_batch;
pub mod context;
pub mod handlers;
pub mod interpreter;

pub use actions::{
    builtin_handlers as builtin_action_handlers, builtin_node_schemas, core_event_schemas,
    CORE_NODE_PACK,
};
pub use context::{ExecutionContext, ExecutionTraceStep};
pub use handlers::{ActionHandler, HandlerError, HandlerRegistry, NodeCall};
pub use interpreter::{builtin_handlers, Interpreter, RuntimeError};

#[cfg(test)]
mod tests {
    use super::*;
    use kvl_core::*;

    #[test]
    fn test_execute_coin_pickup_scenario() {
        let mut graph = Graph::new("coin_pickup", "Scénario Pièce", "Test d'exécution");
        graph.add_variable("score", Value::Int(0));

        // 1. Event
        let event_node = Node::new(
            "node_overlap",
            "Contact Pièce",
            Category::Interactions,
            "",
            NodeKind::Event {
                event_type: "OnPlayerOverlap".into(),
            },
        ).with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        // 2. Action Score
        let score_node = Node::new(
            "node_score",
            "Ajout Score",
            Category::Progression,
            "",
            NodeKind::Action {
                action_type: "AddScore".into(),
            },
        )
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        // 3. Action Destroy
        let destroy_node = Node::new(
            "node_destroy",
            "Détruire Pièce",
            Category::World,
            "",
            NodeKind::Action {
                action_type: "DestroySelf".into(),
            },
        ).with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph.add_node(event_node);
        graph.add_node(score_node);
        graph.add_node(destroy_node);

        graph.add_connection(Connection::new(
            "c1",
            "node_overlap",
            "out_trigger",
            "node_score",
            "in_trigger",
        ));

        graph.add_connection(Connection::new(
            "c2",
            "node_score",
            "out_trigger",
            "node_destroy",
            "in_trigger",
        ));

        // Initialisation de l'exécution
        let mut ctx = Interpreter::init_context(&graph);
        assert_eq!(ctx.get_var("score"), Some(&Value::Int(0)));

        // Déclenchement de l'événement
        let executed_count = Interpreter::trigger_event(&graph, &mut ctx, "OnPlayerOverlap")
            .expect("Exécution sans erreur");

        // 3 nœuds exécutés (Event -> Score -> Destroy)
        assert_eq!(executed_count, 3);

        // Vérification de la variable incrémentée
        assert_eq!(ctx.get_var("score"), Some(&Value::Int(1)));

        // Vérification des logs de traçabilité
        assert!(ctx.execution_log.len() >= 3);
        println!("Trace d'exécution :");
        for step in &ctx.execution_log {
            println!("  [{}] {} : {}", step.timestamp_us, step.node_name, step.message);
        }
    }

    #[test]
    fn test_condition_branching_true_and_false() {
        let mut graph = Graph::new("test_cond", "Test Condition", "Branchement conditionnel KVL");
        graph.add_variable("score", Value::Int(0));
        graph.add_variable("door_is_open", Value::Bool(false));
        graph.add_variable("access_denied", Value::Bool(false));

        let evt = Node::new("evt_interact", "Event Interact", Category::Interactions, "Event", NodeKind::Event { event_type: "OnDoorInteract".into() })
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        let cond = Node::new("cond_score", "Condition Score >= 50", Category::MathLogic, "Condition", NodeKind::Condition { operator: "ScoreAtLeast50".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
            .with_output(Pin::new_trigger("out_true", "Vrai", PinDirection::Output))
            .with_output(Pin::new_trigger("out_false", "Faux", PinDirection::Output));

        let act_unlock = Node::new("act_open", "Ouvrir Porte", Category::World, "Action", NodeKind::Action { action_type: "ToggleDoor".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        let act_deny = Node::new("act_deny", "Refuser Accès", Category::World, "Action", NodeKind::Action { action_type: "DenyAccess".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph.add_node(evt);
        graph.add_node(cond);
        graph.add_node(act_unlock);
        graph.add_node(act_deny);

        graph.connect_pins("evt_interact", "out_trigger", "cond_score", "in_trigger").unwrap();
        graph.connect_pins("cond_score", "out_true", "act_open", "in_trigger").unwrap();
        graph.connect_pins("cond_score", "out_false", "act_deny", "in_trigger").unwrap();

        // CAS 1 : Score = 20 (< 50) -> Branche FAUX activée
        let mut ctx = Interpreter::init_context(&graph);
        ctx.set_var("score", Value::Int(20));

        let steps = Interpreter::trigger_event(&graph, &mut ctx, "OnDoorInteract").unwrap();
        assert_eq!(steps, 3); // Event -> Condition -> DenyAccess
        assert_eq!(ctx.get_var("access_denied"), Some(&Value::Bool(true)), "Accès doit être refusé");
        assert_eq!(ctx.get_var("door_is_open"), Some(&Value::Bool(false)), "Porte doit rester fermée");

        // CAS 2 : Score = 65 (>= 50) -> Branche VRAI activée
        ctx.set_var("score", Value::Int(65));
        ctx.set_var("access_denied", Value::Bool(false));

        let steps2 = Interpreter::trigger_event(&graph, &mut ctx, "OnDoorInteract").unwrap();
        assert_eq!(steps2, 3); // Event -> Condition -> ToggleDoor
        assert_eq!(ctx.get_var("door_is_open"), Some(&Value::Bool(true)), "Porte doit être ouverte");
        assert_eq!(ctx.get_var("access_denied"), Some(&Value::Bool(false)), "Pas de refus d'accès");
    }

    #[test]
    fn test_teleport_and_speed_alteration_actions() {
        let mut graph = Graph::new("test_teleport_speed", "Test Teleport & Speed", "");
        let evt = Node::new("evt", "Event", Category::Interactions, "", NodeKind::Event { event_type: "OnPortal".into() })
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
        let act_teleport = Node::new("act_tp", "Teleport", Category::Movement, "", NodeKind::Action { action_type: "TeleportTo:350,220".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
        let act_speed = Node::new("act_spd", "Slow", Category::Movement, "", NodeKind::Action { action_type: "ModifySpeed:-50".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph.add_node(evt);
        graph.add_node(act_teleport);
        graph.add_node(act_speed);

        graph.connect_pins("evt", "out_trigger", "act_tp", "in_trigger").unwrap();
        graph.connect_pins("act_tp", "out_trigger", "act_spd", "in_trigger").unwrap();

        let mut ctx = ExecutionContext::new();
        ctx.set_var("speed_boost", Value::Int(100));

        let steps = Interpreter::trigger_event(&graph, &mut ctx, "OnPortal").unwrap();
        assert_eq!(steps, 3);
        assert_eq!(ctx.get_var("teleport_target_x"), Some(&Value::Int(350)));
        assert_eq!(ctx.get_var("teleport_target_y"), Some(&Value::Int(220)));
        assert_eq!(ctx.get_var("speed_boost"), Some(&Value::Int(50))); // 100 - 50 = 50
    }

    #[test]
    fn test_play_animation_2d_actions() {
        let mut graph = Graph::new("test_anim", "Test Animation 2D", "");
        let evt = Node::new("evt", "Event", Category::Interactions, "", NodeKind::Event { event_type: "OnIdle".into() })
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
        let act = Node::new("act_anim", "Play walk", Category::Movement, "", NodeKind::Action { action_type: "PlayAnimation2D:walk".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));
        graph.add_node(evt);
        graph.add_node(act);
        graph.connect_pins("evt", "out_trigger", "act_anim", "in_trigger").unwrap();

        let mut ctx = ExecutionContext::new();
        Interpreter::trigger_event(&graph, &mut ctx, "OnIdle").unwrap();
        assert_eq!(ctx.get_var("anim_request"), Some(&Value::String("walk".into())));
        assert_eq!(ctx.get_var("anim_request_entity"), Some(&Value::String(String::new())));
    }

    #[test]
    fn test_play_animation_2d_on_specific_tag() {
        let mut graph = Graph::new("test_anim_on", "Test Animation 2D On", "");
        let evt = Node::new("evt", "Event", Category::Interactions, "", NodeKind::Event { event_type: "OnAlert".into() })
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
        let act = Node::new("act_anim", "Sentinelle attack", Category::Movement, "", NodeKind::Action { action_type: "PlayAnimation2DOn:sentinel:attack".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));
        graph.add_node(evt);
        graph.add_node(act);
        graph.connect_pins("evt", "out_trigger", "act_anim", "in_trigger").unwrap();

        let mut ctx = ExecutionContext::new();
        Interpreter::trigger_event(&graph, &mut ctx, "OnAlert").unwrap();
        assert_eq!(ctx.get_var("anim_request"), Some(&Value::String("attack".into())));
        assert_eq!(ctx.get_var("anim_request_entity"), Some(&Value::String("sentinel".into())));
    }

    #[test]
    fn test_counter_gate_and_variable_conditions() {
        let mut graph = Graph::new("test_counter", "Test Counter & Var", "");
        let evt = Node::new("evt", "Key Collect", Category::Interactions, "", NodeKind::Event { event_type: "OnKeyFound".into() })
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
        let act_inc = Node::new("act_inc", "Inc Keys", Category::Progression, "", NodeKind::Action { action_type: "IncrementCounter:keys".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
        let cond_keys = Node::new("cond_keys", "Condition Keys >= 3", Category::MathLogic, "", NodeKind::Condition { operator: "CounterAtLeast:keys:3".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
            .with_output(Pin::new_trigger("out_true", "Vrai", PinDirection::Output))
            .with_output(Pin::new_trigger("out_false", "Faux", PinDirection::Output));
        let act_win = Node::new("act_win", "Unlock Safe", Category::Progression, "", NodeKind::Action { action_type: "SetVar:vault_open:1".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph.add_node(evt);
        graph.add_node(act_inc);
        graph.add_node(cond_keys);
        graph.add_node(act_win);

        graph.connect_pins("evt", "out_trigger", "act_inc", "in_trigger").unwrap();
        graph.connect_pins("act_inc", "out_trigger", "cond_keys", "in_trigger").unwrap();
        graph.connect_pins("cond_keys", "out_true", "act_win", "in_trigger").unwrap();

        let mut ctx = ExecutionContext::new();
        ctx.set_var("keys", Value::Int(0));

        // Clé 1
        Interpreter::trigger_event(&graph, &mut ctx, "OnKeyFound").unwrap();
        assert_eq!(ctx.get_var("keys"), Some(&Value::Int(1)));
        assert!(ctx.get_var("vault_open").is_none());

        // Clé 2
        Interpreter::trigger_event(&graph, &mut ctx, "OnKeyFound").unwrap();
        assert_eq!(ctx.get_var("keys"), Some(&Value::Int(2)));
        assert!(ctx.get_var("vault_open").is_none());

        // Clé 3 -> Déverrouillage coffre !
        Interpreter::trigger_event(&graph, &mut ctx, "OnKeyFound").unwrap();
        assert_eq!(ctx.get_var("keys"), Some(&Value::Int(3)));
        assert_eq!(ctx.get_var("vault_open"), Some(&Value::Int(1)));
    }

    #[test]
    fn test_dialogue_and_quest_actions_and_conditions() {
        let mut graph = Graph::new("test_dialogue_quest", "Dialogue & Quest", "Test Dialogue & Quête");

        // Event : OnTalk
        let evt = Node::new("evt_talk", "Parler au Gardien", Category::Interactions, "", NodeKind::Event { event_type: "OnTalk".into() })
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        // Action : ShowDialogueChoice
        let act_dialogue = Node::new("act_dlg", "Dialogue", Category::Interactions, "", NodeKind::Action {
            action_type: "ShowDialogueChoice:Gardien:Bienvenue voyageur ! Acceptez-vous la quete ?:Oui:Non".into(),
        })
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        // Action : SetQuest
        let act_quest = Node::new("act_quest", "Start Quest", Category::Progression, "", NodeKind::Action {
            action_type: "SetQuest:main_quest:1".into(),
        })
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        // Action : AdvanceQuest
        let act_adv = Node::new("act_adv", "Advance Quest", Category::Progression, "", NodeKind::Action {
            action_type: "AdvanceQuest:main_quest".into(),
        })
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph.add_node(evt);
        graph.add_node(act_dialogue);
        graph.add_node(act_quest);
        graph.add_node(act_adv);

        graph.connect_pins("evt_talk", "out_trigger", "act_dlg", "in_trigger").unwrap();
        graph.connect_pins("act_dlg", "out_trigger", "act_quest", "in_trigger").unwrap();
        graph.connect_pins("act_quest", "out_trigger", "act_adv", "in_trigger").unwrap();

        let mut ctx = ExecutionContext::new();
        let steps = Interpreter::trigger_event(&graph, &mut ctx, "OnTalk").unwrap();
        assert_eq!(steps, 4);

        // Vérification dialogue
        assert_eq!(ctx.get_var("dialogue_active"), Some(&Value::Bool(true)));
        assert_eq!(ctx.get_var("dialogue_speaker"), Some(&Value::String("Gardien".into())));
        assert_eq!(ctx.get_var("dialogue_choice_1"), Some(&Value::String("Oui".into())));
        assert_eq!(ctx.get_var("dialogue_choice_2"), Some(&Value::String("Non".into())));

        // Vérification quête : étape 1 puis avancée à 2
        assert_eq!(ctx.get_var("quest_main_quest"), Some(&Value::Int(2)));

        // Test de conditions
        let mut graph_cond = Graph::new("test_cond_dlg", "Test Conditions", "");
        let evt_chk = Node::new("evt_chk", "Check", Category::Interactions, "", NodeKind::Event { event_type: "OnCheck".into() })
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
        let cond_open = Node::new("cond_open", "Is Open", Category::MathLogic, "", NodeKind::Condition { operator: "IsDialogueOpen".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
            .with_output(Pin::new_trigger("out_true", "Vrai", PinDirection::Output))
            .with_output(Pin::new_trigger("out_false", "Faux", PinDirection::Output));
        let act_flag = Node::new("act_flag", "Set Flag", Category::Progression, "", NodeKind::Action { action_type: "SetVar:open_confirmed:1".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph_cond.add_node(evt_chk);
        graph_cond.add_node(cond_open);
        graph_cond.add_node(act_flag);

        graph_cond.connect_pins("evt_chk", "out_trigger", "cond_open", "in_trigger").unwrap();
        graph_cond.connect_pins("cond_open", "out_true", "act_flag", "in_trigger").unwrap();

        let steps_cond = Interpreter::trigger_event(&graph_cond, &mut ctx, "OnCheck").unwrap();
        assert_eq!(steps_cond, 3);
        assert_eq!(ctx.get_var("open_confirmed"), Some(&Value::Int(1)));
    }

    #[test]
    fn test_inventory_and_hotbar_kvl_actions_and_conditions() {
        let mut graph = Graph::new("test_inv", "Test Inventory Logic", "");
        let evt = Node::new("evt_chest", "Chest Open", Category::Interactions, "", NodeKind::Event { event_type: "OnChestOpened".into() })
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
        let act_give_pot = Node::new("act_give", "Give Potions", Category::Progression, "", NodeKind::Action { action_type: "GiveItem:potion_heal:3".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
        let cond_has = Node::new("cond_has", "Has Potion", Category::MathLogic, "", NodeKind::Condition { operator: "HasItem:potion_heal".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
            .with_output(Pin::new_trigger("out_true", "Vrai", PinDirection::Output));
        let act_give_key = Node::new("act_key", "Give Key", Category::Progression, "", NodeKind::Action { action_type: "GiveItem:key_dungeon:1".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
        let act_select_slot = Node::new("act_slot", "Select Slot 2", Category::World, "", NodeKind::Action { action_type: "SelectHotbarSlot:2".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph.add_node(evt);
        graph.add_node(act_give_pot);
        graph.add_node(cond_has);
        graph.add_node(act_give_key);
        graph.add_node(act_select_slot);

        graph.connect_pins("evt_chest", "out_trigger", "act_give", "in_trigger").unwrap();
        graph.connect_pins("act_give", "out_trigger", "cond_has", "in_trigger").unwrap();
        graph.connect_pins("cond_has", "out_true", "act_key", "in_trigger").unwrap();
        graph.connect_pins("act_key", "out_trigger", "act_slot", "in_trigger").unwrap();

        let mut ctx = ExecutionContext::new();
        let steps = Interpreter::trigger_event(&graph, &mut ctx, "OnChestOpened").unwrap();
        assert_eq!(steps, 5);

        // Vérification contextuelle
        assert_eq!(ctx.get_var("item_count_potion_heal"), Some(&Value::Int(3)));
        assert_eq!(ctx.get_var("has_potion_heal"), Some(&Value::Bool(true)));
        assert_eq!(ctx.get_var("item_count_key_dungeon"), Some(&Value::Int(1)));
        assert_eq!(ctx.get_var("has_key_dungeon"), Some(&Value::Bool(true)));
        assert_eq!(ctx.get_var("hotbar_selected_slot"), Some(&Value::Int(2)));

        // Test condition HotbarSlotEq et RemoveItem
        let mut graph2 = Graph::new("test_inv2", "Test Inventory 2", "");
        let evt2 = Node::new("evt2", "Event", Category::Interactions, "", NodeKind::Event { event_type: "OnUse".into() })
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
        let cond_slot = Node::new("cond_slot", "Slot is 2", Category::MathLogic, "", NodeKind::Condition { operator: "HotbarSlotEq:2".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
            .with_output(Pin::new_trigger("out_true", "Vrai", PinDirection::Output));
        let act_remove = Node::new("act_rem", "Remove 1 potion", Category::Progression, "", NodeKind::Action { action_type: "RemoveItem:potion_heal:1".into() })
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph2.add_node(evt2);
        graph2.add_node(cond_slot);
        graph2.add_node(act_remove);

        graph2.connect_pins("evt2", "out_trigger", "cond_slot", "in_trigger").unwrap();
        graph2.connect_pins("cond_slot", "out_true", "act_rem", "in_trigger").unwrap();

        let steps2 = Interpreter::trigger_event(&graph2, &mut ctx, "OnUse").unwrap();
        assert_eq!(steps2, 3);
        assert_eq!(ctx.get_var("item_count_potion_heal"), Some(&Value::Int(2)));
    }

    #[test]
    fn test_boss_fight_phase_transitions_and_telegraph_actions() {
        let mut graph = Graph::new("test_boss", "Test Boss Logic", "Scénario de combat de Boss KVL");

        // Event : OnBossPhaseChange
        let evt_phase = Node::new("evt_phase", "Changement Phase Boss", Category::Interactions, "", NodeKind::Event {
            event_type: "OnBossPhaseChange".into(),
        })
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        // Action : SetBossPhase:2
        let act_p2 = Node::new("act_p2", "Passer Phase 2", Category::Progression, "", NodeKind::Action {
            action_type: "SetBossPhase:2".into(),
        })
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        // Action : TriggerBossTelegraph:TargetedSpread
        let act_tele = Node::new("act_tele", "Armer Télégraphie", Category::World, "", NodeKind::Action {
            action_type: "TriggerBossTelegraph:TargetedSpread".into(),
        })
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        // Condition : BossHpLt:20 -> BossEnrage
        let cond_enrage = Node::new("cond_enrage", "Boss PV < 20", Category::MathLogic, "", NodeKind::Condition {
            operator: "BossHpLt:20".into(),
        })
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
        .with_output(Pin::new_trigger("out_true", "Vrai", PinDirection::Output))
        .with_output(Pin::new_trigger("out_false", "Faux", PinDirection::Output));

        let act_enrage = Node::new("act_enrage", "Activer Overdrive", Category::Progression, "", NodeKind::Action {
            action_type: "BossEnrage".into(),
        })
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph.add_node(evt_phase);
        graph.add_node(act_p2);
        graph.add_node(act_tele);
        graph.add_node(cond_enrage);
        graph.add_node(act_enrage);

        graph.connect_pins("evt_phase", "out_trigger", "act_p2", "in_trigger").unwrap();
        graph.connect_pins("act_p2", "out_trigger", "act_tele", "in_trigger").unwrap();
        graph.connect_pins("act_tele", "out_trigger", "cond_enrage", "in_trigger").unwrap();
        graph.connect_pins("cond_enrage", "out_true", "act_enrage", "in_trigger").unwrap();

        let mut ctx = ExecutionContext::new();
        ctx.set_var("boss_hp", Value::Int(15)); // HP bas (< 20)
        ctx.set_var("boss_phase", Value::Int(1));

        let steps = Interpreter::trigger_event(&graph, &mut ctx, "OnBossPhaseChange").unwrap();
        assert_eq!(steps, 5);
        assert_eq!(ctx.get_var("boss_phase"), Some(&Value::Int(2)));
        assert_eq!(ctx.get_var("boss_telegraph_attack"), Some(&Value::String("TargetedSpread".into())));
        assert_eq!(ctx.get_var("boss_enraged"), Some(&Value::Bool(true)));

        // Test de défaite
        let mut defeat_graph = Graph::new("test_defeat", "Test Boss Defeat", "");
        let evt_def = Node::new("evt_def", "Boss Mort", Category::Interactions, "", NodeKind::Event {
            event_type: "OnBossDefeated".into(),
        })
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));
        let act_def = Node::new("act_def", "Victoire", Category::Progression, "", NodeKind::Action {
            action_type: "SetBossDefeated".into(),
        })
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        defeat_graph.add_node(evt_def);
        defeat_graph.add_node(act_def);
        defeat_graph.connect_pins("evt_def", "out_trigger", "act_def", "in_trigger").unwrap();

        Interpreter::trigger_event(&defeat_graph, &mut ctx, "OnBossDefeated").unwrap();
        assert_eq!(ctx.get_var("boss_defeated"), Some(&Value::Bool(true)));
        assert_eq!(ctx.get_var("boss_phase"), Some(&Value::Int(4)));
    }

    #[test]
    fn test_navigation_kvl_actions_and_conditions() {
        let mut graph = Graph::new("test_navigation", "Test Navigation", "Test IA Navigation A*");

        // Event : OnAlert
        let evt = Node::new("evt_alert", "Alerte Ennemi", Category::Interactions, "", NodeKind::Event {
            event_type: "OnAlert".into(),
        })
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        // Action : NavChasePlayer
        let act_chase = Node::new("act_chase", "Traquer Joueur", Category::ArtificialIntelligence, "", NodeKind::Action {
            action_type: "NavChasePlayer".into(),
        })
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        // Condition : IsNavigating
        let cond_nav = Node::new("cond_nav", "En Navigation ?", Category::MathLogic, "", NodeKind::Condition {
            operator: "IsNavigating".into(),
        })
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
        .with_output(Pin::new_trigger("out_true", "True", PinDirection::Output))
        .with_output(Pin::new_trigger("out_false", "False", PinDirection::Output));

        // Action : NavGoTo (si out_true)
        let act_goto = Node::new("act_goto", "Aller Cible", Category::Movement, "", NodeKind::Action {
            action_type: "NavGoTo:250,180".into(),
        })
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph.add_node(evt);
        graph.add_node(act_chase);
        graph.add_node(cond_nav);
        graph.add_node(act_goto);

        graph.connect_pins("evt_alert", "out_trigger", "act_chase", "in_trigger").unwrap();
        graph.connect_pins("act_chase", "out_trigger", "cond_nav", "in_trigger").unwrap();
        graph.connect_pins("cond_nav", "out_true", "act_goto", "in_trigger").unwrap();

        let mut ctx = ExecutionContext::new();
        let steps = Interpreter::trigger_event(&graph, &mut ctx, "OnAlert").unwrap();
        assert_eq!(steps, 4, "Les 4 nœuds doivent être exécutés séquentiellement");

        assert_eq!(ctx.get_var("nav_mode"), Some(&Value::String("GoTo:250,180".into())));
        assert_eq!(ctx.get_var("nav_target_x"), Some(&Value::Int(250)));
        assert_eq!(ctx.get_var("nav_target_y"), Some(&Value::Int(180)));
    }

    #[test]
    fn test_swarm_flocking_kvl_actions_and_conditions() {
        let mut graph = Graph::new("test_swarm", "Test Swarm Logic", "");

        let evt = Node::new("evt_scatter", "On Bomb", Category::Interactions, "", NodeKind::Event {
            event_type: "OnBombExplode".into(),
        })
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        let act_scatter = Node::new("act_scatter", "Scatter Swarm", Category::Movement, "", NodeKind::Action {
            action_type: "SwarmScatter:1,3.5".into(),
        })
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        let cond_scatter = Node::new("cond_scatter", "Is Scattered ?", Category::MathLogic, "", NodeKind::Condition {
            operator: "SwarmIsScattered:1".into(),
        })
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
        .with_output(Pin::new_trigger("out_true", "True", PinDirection::Output));

        let act_target = Node::new("act_target", "Target Point", Category::Movement, "", NodeKind::Action {
            action_type: "SwarmSetTarget:1,400,300".into(),
        })
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph.add_node(evt);
        graph.add_node(act_scatter);
        graph.add_node(cond_scatter);
        graph.add_node(act_target);

        graph.connect_pins("evt_scatter", "out_trigger", "act_scatter", "in_trigger").unwrap();
        graph.connect_pins("act_scatter", "out_trigger", "cond_scatter", "in_trigger").unwrap();
        graph.connect_pins("cond_scatter", "out_true", "act_target", "in_trigger").unwrap();

        let mut ctx = ExecutionContext::new();
        let steps = Interpreter::trigger_event(&graph, &mut ctx, "OnBombExplode").unwrap();
        assert_eq!(steps, 4);

        assert_eq!(ctx.get_var("swarm_is_scattered_1"), Some(&Value::Bool(true)));
        assert_eq!(ctx.get_var("swarm_scatter_1"), Some(&Value::Float(3.5)));
        assert_eq!(ctx.get_var("swarm_target_x_1"), Some(&Value::Int(400)));
        assert_eq!(ctx.get_var("swarm_target_y_1"), Some(&Value::Int(300)));
    }

    #[test]
    fn migrated_actions_preserve_legacy_trace_and_variables() {
        let mut graph = Graph::new("g_migrated", "Migration", "AddScore + DamagePlayer + GiveItem");

        let event = Node::new(
            "e",
            "Contact",
            Category::Interactions,
            "",
            NodeKind::Event {
                event_type: "OnPlayerOverlap".into(),
            },
        )
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        let score = Node::new(
            "a1",
            "Score",
            Category::Progression,
            "",
            NodeKind::Action {
                action_type: "AddScore".into(),
            },
        )
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        let damage = Node::new(
            "a2",
            "Dégâts 2",
            Category::Combat,
            "",
            NodeKind::Action {
                action_type: "DamagePlayer:2".into(),
            },
        )
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input))
        .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output));

        let give = Node::new(
            "a3",
            "Deux pièces",
            Category::World,
            "",
            NodeKind::Action {
                action_type: "GiveItem:coin:2".into(),
            },
        )
        .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input));

        graph.add_node(event);
        graph.add_node(score);
        graph.add_node(damage);
        graph.add_node(give);
        graph.connect_pins("e", "out_trigger", "a1", "in_trigger").unwrap();
        graph.connect_pins("a1", "out_trigger", "a2", "in_trigger").unwrap();
        graph.connect_pins("a2", "out_trigger", "a3", "in_trigger").unwrap();

        let mut ctx = ExecutionContext::new();
        ctx.set_var("player_hp", Value::Int(3));

        let steps = Interpreter::trigger_event(&graph, &mut ctx, "OnPlayerOverlap").unwrap();
        assert_eq!(steps, 4);
        assert_eq!(ctx.get_var("score"), Some(&Value::Int(1)));
        assert_eq!(ctx.get_var("player_hp"), Some(&Value::Int(1)));
        assert_eq!(ctx.get_var("item_count_coin"), Some(&Value::Int(2)));
        assert_eq!(ctx.get_var("has_coin"), Some(&Value::Bool(true)));

        let messages: Vec<&str> = ctx
            .execution_log
            .iter()
            .map(|step| step.message.as_str())
            .collect();
        assert!(messages.iter().any(|m| m.contains("-2 PV")));
        assert!(messages.iter().any(|m| m.contains("Objet 'coin'")));
    }

    #[test]
    fn every_action_in_asset_graphs_is_catalogued_and_handled() {
        use std::fs;
        use std::path::Path;

        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scripts");
        if !dir.is_dir() {
            return;
        }

        let handlers = builtin_handlers();
        let mut schemas = kvl_core::NodeRegistry::new();
        for schema in builtin_node_schemas() {
            schemas.register(schema).expect("schéma unique");
        }

        let mut inspected = 0;
        for entry in fs::read_dir(&dir).expect("dossier des scripts") {
            let path = entry.expect("entrée").path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let raw = fs::read_to_string(&path).expect("lecture du graphe");
            let graph = match Graph::from_json(&raw) {
                Ok(graph) => graph,
                Err(_) => continue,
            };
            for node in graph.nodes.values() {
                if let NodeKind::Action { action_type } = &node.kind {
                    assert!(
                        handlers.is_registered(action_type),
                        "handler manquant pour '{}' ({})",
                        action_type,
                        path.display()
                    );
                    assert!(
                        schemas.resolve(action_type).is_some(),
                        "schéma manquant pour '{}' ({})",
                        action_type,
                        path.display()
                    );
                    inspected += 1;
                }
            }
        }

        assert!(inspected > 0, "aucune action inspectée");
    }

    #[test]
    fn custom_registry_can_override_a_builtin_action() {
        fn jackpot(ctx: &mut ExecutionContext, _call: &NodeCall) -> Result<(), RuntimeError> {
            ctx.set_var("score", Value::Int(999));
            Ok(())
        }

        let mut handlers = builtin_handlers().clone();
        handlers.replace("AddScore", jackpot);

        let mut graph = Graph::new("g_override", "Override", "");
        graph.add_node(
            Node::new(
                "e",
                "Contact",
                Category::Interactions,
                "",
                NodeKind::Event {
                    event_type: "OnPlayerOverlap".into(),
                },
            )
            .with_output(Pin::new_trigger("out_trigger", "Out", PinDirection::Output)),
        );
        graph.add_node(
            Node::new(
                "a1",
                "Score",
                Category::Progression,
                "",
                NodeKind::Action {
                    action_type: "AddScore".into(),
                },
            )
            .with_input(Pin::new_trigger("in_trigger", "In", PinDirection::Input)),
        );
        graph.connect_pins("e", "out_trigger", "a1", "in_trigger").unwrap();

        let mut ctx = ExecutionContext::new();
        Interpreter::trigger_event_with(&graph, &mut ctx, "OnPlayerOverlap", &handlers).unwrap();
        assert_eq!(ctx.get_var("score"), Some(&Value::Int(999)));
    }
}
