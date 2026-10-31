//! Actions KVL historiques, migrées vers le registre de handlers.
//!
//! Ce module a été extrait mécaniquement du `match` de l'interpréteur afin de
//! préserver **exactement** les variables, les messages de trace et les paramètres
//! préfixés. Il reste à découper par catégorie (mouvement, combat, monde, IA) et à
//! documenter chaque handler ; c'est un refactor de forme, pas de comportement.
//!
//! Les trois actions du pack cœur (`AddScore`, `DamagePlayer`, `GiveItem`) ne sont
//! pas ici : elles ont été réécrites dans `actions.rs` avec des paramètres explicites.

use kvl_core::{Category, NodeSchema, Value};

use crate::actions::CORE_NODE_PACK;
use crate::context::ExecutionContext;
use crate::handlers::{HandlerRegistry, NodeCall};
use crate::interpreter::RuntimeError;

/// Enregistre toutes les actions historiques migrées.
pub(crate) fn register(registry: &mut HandlerRegistry) {
    registry
        .register("BonusPoints", h_bonus_points)
        .expect("handler unique : BonusPoints");
    registry
        .register("BoostSpeed", h_boost_speed)
        .expect("handler unique : BoostSpeed");
    registry
        .register("SpawnSparkles", h_spawn_sparkles)
        .expect("handler unique : SpawnSparkles");
    registry
        .register("ToggleDoor", h_toggle_door)
        .expect("handler unique : ToggleDoor");
    registry
        .register("DenyAccess", h_deny_access)
        .expect("handler unique : DenyAccess");
    registry
        .register("TriggerAlarm", h_trigger_alarm)
        .expect("handler unique : TriggerAlarm");
    registry
        .register("ClearAlarm", h_clear_alarm)
        .expect("handler unique : ClearAlarm");
    registry
        .register("LockAllDoors", h_lock_all_doors)
        .expect("handler unique : LockAllDoors");
    registry
        .register("ResetScore", h_reset_score)
        .expect("handler unique : ResetScore");
    registry
        .register("HealPlayer", h_heal_player)
        .expect("handler unique : HealPlayer");
    registry
        .register("HealPlayer:", h_heal_player_2)
        .expect("handler unique : HealPlayer:");
    registry
        .register_prefix("HealPlayer:", h_heal_player_2)
        .expect("préfixe unique : HealPlayer:");
    registry
        .register("TeleportPlayer", h_teleport_player)
        .expect("handler unique : TeleportPlayer");
    registry
        .register("TeleportTo:", h_teleport_to)
        .expect("handler unique : TeleportTo:");
    registry
        .register_prefix("TeleportTo:", h_teleport_to)
        .expect("préfixe unique : TeleportTo:");
    registry
        .register("SlowSpeed", h_slow_speed)
        .expect("handler unique : SlowSpeed");
    registry
        .register("ModifySpeed:", h_modify_speed)
        .expect("handler unique : ModifySpeed:");
    registry
        .register_prefix("ModifySpeed:", h_modify_speed)
        .expect("préfixe unique : ModifySpeed:");
    registry
        .register("NavChasePlayer", h_nav_chase_player)
        .expect("handler unique : NavChasePlayer");
    registry
        .register("ChasePlayer", h_nav_chase_player)
        .expect("handler unique : ChasePlayer");
    registry
        .register("NavStop", h_nav_stop)
        .expect("handler unique : NavStop");
    registry
        .register("StopNavigation", h_nav_stop)
        .expect("handler unique : StopNavigation");
    registry
        .register("NavGoTo:", h_nav_go_to)
        .expect("handler unique : NavGoTo:");
    registry
        .register("NavigateTo:", h_nav_go_to)
        .expect("handler unique : NavigateTo:");
    registry
        .register_prefix("NavGoTo:", h_nav_go_to)
        .expect("préfixe unique : NavGoTo:");
    registry
        .register_prefix("NavigateTo:", h_nav_go_to)
        .expect("préfixe unique : NavigateTo:");
    registry
        .register("SwarmScatter", h_swarm_scatter)
        .expect("handler unique : SwarmScatter");
    registry
        .register("SwarmScatter:", h_swarm_scatter_2)
        .expect("handler unique : SwarmScatter:");
    registry
        .register_prefix("SwarmScatter:", h_swarm_scatter_2)
        .expect("préfixe unique : SwarmScatter:");
    registry
        .register("SwarmTargetPlayer", h_swarm_target_player)
        .expect("handler unique : SwarmTargetPlayer");
    registry
        .register("SwarmTargetPlayer:", h_swarm_target_player_2)
        .expect("handler unique : SwarmTargetPlayer:");
    registry
        .register_prefix("SwarmTargetPlayer:", h_swarm_target_player_2)
        .expect("préfixe unique : SwarmTargetPlayer:");
    registry
        .register("SwarmSetTarget:", h_swarm_set_target)
        .expect("handler unique : SwarmSetTarget:");
    registry
        .register_prefix("SwarmSetTarget:", h_swarm_set_target)
        .expect("préfixe unique : SwarmSetTarget:");
    registry
        .register("SwarmSetCohesion:", h_swarm_set_cohesion)
        .expect("handler unique : SwarmSetCohesion:");
    registry
        .register_prefix("SwarmSetCohesion:", h_swarm_set_cohesion)
        .expect("préfixe unique : SwarmSetCohesion:");
    registry
        .register("IncrementCounter:", h_increment_counter)
        .expect("handler unique : IncrementCounter:");
    registry
        .register_prefix("IncrementCounter:", h_increment_counter)
        .expect("préfixe unique : IncrementCounter:");
    registry
        .register("AddVar:", h_add_var)
        .expect("handler unique : AddVar:");
    registry
        .register_prefix("AddVar:", h_add_var)
        .expect("préfixe unique : AddVar:");
    registry
        .register("SetVar:", h_set_var)
        .expect("handler unique : SetVar:");
    registry
        .register_prefix("SetVar:", h_set_var)
        .expect("préfixe unique : SetVar:");
    registry
        .register("DestroySelf", h_destroy_self)
        .expect("handler unique : DestroySelf");
    registry
        .register("EmitBurst", h_emit_burst)
        .expect("handler unique : EmitBurst");
    registry
        .register("EmitBurst:", h_emit_burst_2)
        .expect("handler unique : EmitBurst:");
    registry
        .register_prefix("EmitBurst:", h_emit_burst_2)
        .expect("préfixe unique : EmitBurst:");
    registry
        .register("ToggleEmitter", h_toggle_emitter)
        .expect("handler unique : ToggleEmitter");
    registry
        .register("ToggleEmitter:", h_toggle_emitter_2)
        .expect("handler unique : ToggleEmitter:");
    registry
        .register_prefix("ToggleEmitter:", h_toggle_emitter_2)
        .expect("préfixe unique : ToggleEmitter:");
    registry
        .register("ShowDialogue:", h_show_dialogue)
        .expect("handler unique : ShowDialogue:");
    registry
        .register_prefix("ShowDialogue:", h_show_dialogue)
        .expect("préfixe unique : ShowDialogue:");
    registry
        .register("ShowDialogueChoice:", h_show_dialogue_choice)
        .expect("handler unique : ShowDialogueChoice:");
    registry
        .register_prefix("ShowDialogueChoice:", h_show_dialogue_choice)
        .expect("préfixe unique : ShowDialogueChoice:");
    registry
        .register("CloseDialogue", h_close_dialogue)
        .expect("handler unique : CloseDialogue");
    registry
        .register("SetQuest:", h_set_quest)
        .expect("handler unique : SetQuest:");
    registry
        .register_prefix("SetQuest:", h_set_quest)
        .expect("préfixe unique : SetQuest:");
    registry
        .register("AdvanceQuest:", h_advance_quest)
        .expect("handler unique : AdvanceQuest:");
    registry
        .register_prefix("AdvanceQuest:", h_advance_quest)
        .expect("préfixe unique : AdvanceQuest:");
    registry
        .register("CompleteQuest:", h_complete_quest)
        .expect("handler unique : CompleteQuest:");
    registry
        .register_prefix("CompleteQuest:", h_complete_quest)
        .expect("préfixe unique : CompleteQuest:");
    registry
        .register("RemoveItem:", h_remove_item)
        .expect("handler unique : RemoveItem:");
    registry
        .register_prefix("RemoveItem:", h_remove_item)
        .expect("préfixe unique : RemoveItem:");
    registry
        .register("SelectHotbarSlot:", h_select_hotbar_slot)
        .expect("handler unique : SelectHotbarSlot:");
    registry
        .register_prefix("SelectHotbarSlot:", h_select_hotbar_slot)
        .expect("préfixe unique : SelectHotbarSlot:");
    registry
        .register("UseItem:", h_use_item)
        .expect("handler unique : UseItem:");
    registry
        .register_prefix("UseItem:", h_use_item)
        .expect("préfixe unique : UseItem:");
    registry
        .register("SetBossPhase:", h_set_boss_phase)
        .expect("handler unique : SetBossPhase:");
    registry
        .register_prefix("SetBossPhase:", h_set_boss_phase)
        .expect("préfixe unique : SetBossPhase:");
    registry
        .register("BossEnrage", h_boss_enrage)
        .expect("handler unique : BossEnrage");
    registry
        .register("SetBossDefeated", h_set_boss_defeated)
        .expect("handler unique : SetBossDefeated");
    registry
        .register("TriggerBossTelegraph:", h_trigger_boss_telegraph)
        .expect("handler unique : TriggerBossTelegraph:");
    registry
        .register_prefix("TriggerBossTelegraph:", h_trigger_boss_telegraph)
        .expect("préfixe unique : TriggerBossTelegraph:");
    registry
        .register("TriggerBossRoar", h_trigger_boss_roar)
        .expect("handler unique : TriggerBossRoar");
    registry
        .register("StopAnimation2D", h_stop_animation2_d)
        .expect("handler unique : StopAnimation2D");
    registry
        .register("PlayAnimation2DOn:", h_play_animation2_d_on)
        .expect("handler unique : PlayAnimation2DOn:");
    registry
        .register_prefix("PlayAnimation2DOn:", h_play_animation2_d_on)
        .expect("préfixe unique : PlayAnimation2DOn:");
    registry
        .register("PlayAnimation2D:", h_play_animation2_d)
        .expect("handler unique : PlayAnimation2D:");
    registry
        .register_prefix("PlayAnimation2D:", h_play_animation2_d)
        .expect("préfixe unique : PlayAnimation2D:");
}

fn h_bonus_points(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let current_score = match ctx.get_var("score") {
        Some(Value::Int(s)) => *s,
        _ => 0,
    };
    ctx.set_var("score", Value::Int(current_score + 50));
    ctx.record_step(call.node_id, call.node_name, "Bonus majeur de +50 points appliqué !");
    Ok(())
}

fn h_boost_speed(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let current_boost = match ctx.get_var("speed_boost") {
        Some(Value::Int(b)) => *b,
        _ => 0,
    };
    ctx.set_var("speed_boost", Value::Int(current_boost + 120));
    ctx.record_step(call.node_id, call.node_name, "Accélération du joueur (+120 px/s) !");
    Ok(())
}

fn h_spawn_sparkles(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let count = match ctx.get_var("sparkle_burst") {
        Some(Value::Int(c)) => *c,
        _ => 0,
    };
    ctx.set_var("sparkle_burst", Value::Int(count + 1));
    ctx.record_step(call.node_id, call.node_name, "Pluie d'étincelles dorées déclenchée !");
    Ok(())
}

fn h_toggle_door(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let is_open = match ctx.get_var("door_is_open") {
        Some(Value::Bool(b)) => *b,
        _ => false,
    };
    ctx.set_var("door_is_open", Value::Bool(!is_open));
    ctx.record_step(
        call.node_id,
        call.node_name,
        if !is_open { "Porte déverrouillée et ouverte !" } else { "Porte refermée." },
    );
    Ok(())
}

fn h_deny_access(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    ctx.set_var("access_denied", Value::Bool(true));
    ctx.record_step(call.node_id, call.node_name, "Accès refusé ! Condition non remplie.");
    Ok(())
}

fn h_trigger_alarm(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    ctx.set_var("alarm_active", Value::Bool(true));
    ctx.record_step(call.node_id, call.node_name, "Alerte de patrouille déclenchée : intrus repéré !");
    Ok(())
}

fn h_clear_alarm(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    ctx.set_var("alarm_active", Value::Bool(false));
    ctx.record_step(call.node_id, call.node_name, "Alerte levée : retour à la patrouille normale.");
    Ok(())
}

fn h_lock_all_doors(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    ctx.set_var("door_is_open", Value::Bool(false));
    ctx.record_step(call.node_id, call.node_name, "Verrouillage de sécurité d'urgence des portes !");
    Ok(())
}

fn h_reset_score(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    ctx.set_var("score", Value::Int(0));
    ctx.record_step(call.node_id, call.node_name, "Score réinitialisé à 0 !");
    Ok(())
}

fn h_heal_player(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let current_hp = match ctx.get_var("player_hp") {
        Some(Value::Int(hp)) => *hp,
        _ => 3,
    };
    let max_hp = match ctx.get_var("player_max_hp") {
        Some(Value::Int(m)) => *m,
        _ => 3,
    };
    let new_hp = (current_hp + 1).min(max_hp);
    ctx.set_var("player_hp", Value::Int(new_hp));
    ctx.record_step(call.node_id, call.node_name, format!("Soin accordé au joueur : +1 PV (Total: {})", new_hp));
    Ok(())
}

fn h_heal_player_2(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let amount = op.trim_start_matches("HealPlayer:").parse::<i64>().unwrap_or(1);
    let current_hp = match ctx.get_var("player_hp") {
        Some(Value::Int(hp)) => *hp,
        _ => 3,
    };
    let max_hp = match ctx.get_var("player_max_hp") {
        Some(Value::Int(m)) => *m,
        _ => 3,
    };
    let new_hp = (current_hp + amount).min(max_hp);
    ctx.set_var("player_hp", Value::Int(new_hp));
    ctx.record_step(call.node_id, call.node_name, format!("Soin accordé au joueur : +{} PV (Total: {})", amount, new_hp));
    Ok(())
}

fn h_teleport_player(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    ctx.set_var("teleport_target_x", Value::Int(120));
    ctx.set_var("teleport_target_y", Value::Int(480));
    ctx.record_step(call.node_id, call.node_name, "Téléportation du joueur vers (120, 480) !");
    Ok(())
}

fn h_teleport_to(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let coords = op.trim_start_matches("TeleportTo:");
    let mut parts = coords.split(',');
    let tx = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(120);
    let ty = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(480);
    ctx.set_var("teleport_target_x", Value::Int(tx));
    ctx.set_var("teleport_target_y", Value::Int(ty));
    ctx.record_step(call.node_id, call.node_name, format!("Téléportation de l'entité vers ({}, {}) !", tx, ty));
    Ok(())
}

fn h_slow_speed(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let current_boost = match ctx.get_var("speed_boost") {
        Some(Value::Int(b)) => *b,
        _ => 0,
    };
    ctx.set_var("speed_boost", Value::Int(current_boost - 80));
    ctx.record_step(call.node_id, call.node_name, "Ralentissement de l'entité (-80 px/s) !");
    Ok(())
}

fn h_modify_speed(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let delta = op.trim_start_matches("ModifySpeed:").parse::<i64>().unwrap_or(0);
    let current_boost = match ctx.get_var("speed_boost") {
        Some(Value::Int(b)) => *b,
        _ => 0,
    };
    ctx.set_var("speed_boost", Value::Int(current_boost + delta));
    ctx.record_step(call.node_id, call.node_name, format!("Vitesse modifiée ({:+} px/s) !", delta));
    Ok(())
}

fn h_nav_chase_player(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    ctx.set_var("nav_mode", Value::String("ChasePlayer".into()));
    ctx.record_step(call.node_id, call.node_name, "IA : Ordre de traque A* du joueur activé !");
    Ok(())
}

fn h_nav_stop(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    ctx.set_var("nav_mode", Value::String("Idle".into()));
    ctx.record_step(call.node_id, call.node_name, "IA : Ordre d'arrêt de navigation.");
    Ok(())
}

fn h_nav_go_to(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let prefix = if op.starts_with("NavGoTo:") { "NavGoTo:" } else { "NavigateTo:" };
    let coords = op.trim_start_matches(prefix);
    let mut parts = coords.split(',');
    let tx = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    let ty = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    ctx.set_var("nav_mode", Value::String(format!("GoTo:{},{}", tx, ty)));
    ctx.set_var("nav_target_x", Value::Int(tx));
    ctx.set_var("nav_target_y", Value::Int(ty));
    ctx.record_step(call.node_id, call.node_name, format!("IA : Navigation A* vers ({}, {}) !", tx, ty));
    Ok(())
}

fn h_swarm_scatter(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    ctx.set_var("swarm_scatter_1", Value::Float(2.0));
    ctx.set_var("swarm_is_scattered_1", Value::Bool(true));
    ctx.record_step(call.node_id, call.node_name, "Meute : Éparpillement d'urgence (Scatter) activé !");
    Ok(())
}

fn h_swarm_scatter_2(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let rest = op.trim_start_matches("SwarmScatter:");
    let mut parts = rest.split(',');
    let swarm_id = parts.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let dur = parts.next().and_then(|s| s.parse::<f64>().ok()).unwrap_or(2.0);
    ctx.set_var(format!("swarm_scatter_{}", swarm_id), Value::Float(dur));
    ctx.set_var(format!("swarm_is_scattered_{}", swarm_id), Value::Bool(true));
    ctx.record_step(call.node_id, call.node_name, format!("Meute #{} : Éparpillement d'urgence ({:.1}s) déclenché !", swarm_id, dur));
    Ok(())
}

fn h_swarm_target_player(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    ctx.set_var("swarm_target_mode_1", Value::String("Player".into()));
    ctx.record_step(call.node_id, call.node_name, "Meute : Traque collective du joueur enclenchée !");
    Ok(())
}

fn h_swarm_target_player_2(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let swarm_id = op.trim_start_matches("SwarmTargetPlayer:").parse::<u32>().unwrap_or(1);
    ctx.set_var(format!("swarm_target_mode_{}", swarm_id), Value::String("Player".into()));
    ctx.record_step(call.node_id, call.node_name, format!("Meute #{} : Traque collective du joueur enclenchée !", swarm_id));
    Ok(())
}

fn h_swarm_set_target(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let rest = op.trim_start_matches("SwarmSetTarget:");
    let mut parts = rest.split(',');
    let swarm_id = parts.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let tx = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    let ty = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    ctx.set_var(format!("swarm_target_x_{}", swarm_id), Value::Int(tx));
    ctx.set_var(format!("swarm_target_y_{}", swarm_id), Value::Int(ty));
    ctx.record_step(call.node_id, call.node_name, format!("Meute #{} : Cible collective assignée en ({}, {})", swarm_id, tx, ty));
    Ok(())
}

fn h_swarm_set_cohesion(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let rest = op.trim_start_matches("SwarmSetCohesion:");
    let mut parts = rest.split(',');
    let swarm_id = parts.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let weight = parts.next().and_then(|s| s.parse::<f64>().ok()).unwrap_or(1.0);
    ctx.set_var(format!("swarm_cohesion_{}", swarm_id), Value::Float(weight));
    ctx.record_step(call.node_id, call.node_name, format!("Meute #{} : Poids de cohésion ajusté à {:.2}", swarm_id, weight));
    Ok(())
}

fn h_increment_counter(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let counter_name = op.trim_start_matches("IncrementCounter:");
    let current_count = match ctx.get_var(counter_name) {
        Some(Value::Int(c)) => *c,
        _ => 0,
    };
    ctx.set_var(counter_name, Value::Int(current_count + 1));
    ctx.record_step(call.node_id, call.node_name, format!("Compteur '{}' incrémenté : {}", counter_name, current_count + 1));
    Ok(())
}

fn h_add_var(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let rest = op.trim_start_matches("AddVar:");
    let mut parts = rest.split(':');
    if let (Some(var_name), Some(delta_str)) = (parts.next(), parts.next()) {
        let delta = delta_str.parse::<i64>().unwrap_or(0);
        let current_val = match ctx.get_var(var_name) {
            Some(Value::Int(v)) => *v,
            _ => 0,
        };
        ctx.set_var(var_name, Value::Int(current_val + delta));
        ctx.record_step(call.node_id, call.node_name, format!("Variable '{}' ajustée de {} -> {}", var_name, delta, current_val + delta));
    }
    Ok(())
}

fn h_set_var(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let rest = op.trim_start_matches("SetVar:");
    let mut parts = rest.split(':');
    if let (Some(var_name), Some(val_str)) = (parts.next(), parts.next()) {
        let val = val_str.parse::<i64>().unwrap_or(0);
        ctx.set_var(var_name, Value::Int(val));
        ctx.record_step(call.node_id, call.node_name, format!("Variable '{}' fixée à {}", var_name, val));
    }
    Ok(())
}

fn h_destroy_self(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    ctx.record_step(call.node_id, call.node_name, "Objet marqué pour destruction");
    Ok(())
}

fn h_emit_burst(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let current = match ctx.get_var("emitter_burst") {
        Some(Value::Int(c)) => *c,
        _ => 0,
    };
    ctx.set_var("emitter_burst", Value::Int(current + 25));
    ctx.record_step(call.node_id, call.node_name, "Gerbe de particules (Burst) déclenchée !");
    Ok(())
}

fn h_emit_burst_2(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let count = op.trim_start_matches("EmitBurst:").parse::<i64>().unwrap_or(25);
    let current = match ctx.get_var("emitter_burst") {
        Some(Value::Int(c)) => *c,
        _ => 0,
    };
    ctx.set_var("emitter_burst", Value::Int(current + count));
    ctx.record_step(call.node_id, call.node_name, format!("Gerbe de {} particules déclenchée !", count));
    Ok(())
}

fn h_toggle_emitter(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let active = match ctx.get_var("emitter_active") {
        Some(Value::Bool(b)) => *b,
        _ => true,
    };
    ctx.set_var("emitter_active", Value::Bool(!active));
    ctx.record_step(call.node_id, call.node_name, if !active { "Émetteur de particules activé !" } else { "Émetteur de particules éteint." });
    Ok(())
}

fn h_toggle_emitter_2(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let state_str = op.trim_start_matches("ToggleEmitter:");
    let state = state_str == "true" || state_str == "1" || state_str == "on";
    ctx.set_var("emitter_active", Value::Bool(state));
    ctx.record_step(call.node_id, call.node_name, if state { "Émetteur de particules allumé !" } else { "Émetteur de particules éteint." });
    Ok(())
}

fn h_show_dialogue(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let rest = op.trim_start_matches("ShowDialogue:");
    let mut parts = rest.splitn(2, ':');
    let speaker = parts.next().unwrap_or("PNJ");
    let text = parts.next().unwrap_or("");
    ctx.set_var("dialogue_active", Value::Bool(true));
    ctx.set_var("dialogue_speaker", Value::String(speaker.to_string()));
    ctx.set_var("dialogue_text", Value::String(text.to_string()));
    ctx.set_var("dialogue_has_choices", Value::Bool(false));
    ctx.set_var("dialogue_choice_1", Value::String(String::new()));
    ctx.set_var("dialogue_choice_2", Value::String(String::new()));
    ctx.set_var("dialogue_selected_choice", Value::Int(0));
    ctx.record_step(call.node_id, call.node_name, format!("Ouverture dialogue [{}] : {}", speaker, text));
    Ok(())
}

fn h_show_dialogue_choice(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let rest = op.trim_start_matches("ShowDialogueChoice:");
    let mut parts = rest.splitn(4, ':');
    let speaker = parts.next().unwrap_or("PNJ");
    let text = parts.next().unwrap_or("");
    let c1 = parts.next().unwrap_or("Option 1");
    let c2 = parts.next().unwrap_or("Option 2");
    ctx.set_var("dialogue_active", Value::Bool(true));
    ctx.set_var("dialogue_speaker", Value::String(speaker.to_string()));
    ctx.set_var("dialogue_text", Value::String(text.to_string()));
    ctx.set_var("dialogue_has_choices", Value::Bool(true));
    ctx.set_var("dialogue_choice_1", Value::String(c1.to_string()));
    ctx.set_var("dialogue_choice_2", Value::String(c2.to_string()));
    ctx.set_var("dialogue_selected_choice", Value::Int(0));
    ctx.record_step(call.node_id, call.node_name, format!("Dialogue à choix [{}] : {} ([1] {}, [2] {})", speaker, text, c1, c2));
    Ok(())
}

fn h_close_dialogue(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    ctx.set_var("dialogue_active", Value::Bool(false));
    ctx.record_step(call.node_id, call.node_name, "Fermeture de la boîte de dialogue");
    Ok(())
}

fn h_set_quest(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let rest = op.trim_start_matches("SetQuest:");
    let mut parts = rest.split(':');
    if let (Some(qid), Some(step_str)) = (parts.next(), parts.next()) {
        let step = step_str.parse::<i64>().unwrap_or(1);
        ctx.set_var(format!("quest_{}", qid), Value::Int(step));
        ctx.record_step(call.node_id, call.node_name, format!("Quête '{}' positionnée à l'étape {}", qid, step));
    }
    Ok(())
}

fn h_advance_quest(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let qid = op.trim_start_matches("AdvanceQuest:");
    let key = format!("quest_{}", qid);
    let current = match ctx.get_var(&key) {
        Some(Value::Int(s)) => *s,
        _ => 0,
    };
    let next = current + 1;
    ctx.set_var(&key, Value::Int(next));
    ctx.record_step(call.node_id, call.node_name, format!("Quête '{}' avancée à l'étape {}", qid, next));
    Ok(())
}

fn h_complete_quest(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let qid = op.trim_start_matches("CompleteQuest:");
    ctx.set_var(format!("quest_{}", qid), Value::Int(100));
    ctx.set_var(format!("quest_completed_{}", qid), Value::Bool(true));
    ctx.record_step(call.node_id, call.node_name, format!("Quête '{}' accomplie avec succès !", qid));
    Ok(())
}

fn h_remove_item(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let rest = op.trim_start_matches("RemoveItem:");
    let mut parts = rest.split(':');
    let item_id = parts.next().unwrap_or("item");
    let count = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(1);
    let current_count = match ctx.get_var(&format!("item_count_{}", item_id)) {
        Some(Value::Int(c)) => *c,
        _ => 0,
    };
    let new_count = (current_count - count).max(0);
    ctx.set_var(format!("item_count_{}", item_id), Value::Int(new_count));
    ctx.set_var(format!("has_{}", item_id), Value::Bool(new_count > 0));
    ctx.record_step(call.node_id, call.node_name, format!("Objet '{}' retiré de l'inventaire (-{}) (Reste: {})", item_id, count, new_count));
    Ok(())
}

fn h_select_hotbar_slot(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let slot_idx = op.trim_start_matches("SelectHotbarSlot:").parse::<i64>().unwrap_or(0);
    ctx.set_var("hotbar_selected_slot", Value::Int(slot_idx));
    ctx.record_step(call.node_id, call.node_name, format!("Emplacement de barre d'action sélectionné : {}", slot_idx));
    Ok(())
}

fn h_use_item(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let item_id = op.trim_start_matches("UseItem:");
    ctx.set_var(format!("used_item_{}", item_id), Value::Bool(true));
    ctx.record_step(call.node_id, call.node_name, format!("Objet '{}' utilisé via KVL !", item_id));
    Ok(())
}

fn h_set_boss_phase(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let phase_str = op.trim_start_matches("SetBossPhase:");
    let phase_val = match phase_str {
        "1" | "Phase1" => 1,
        "2" | "Phase2" => 2,
        "3" | "Phase3" => 3,
        "Defeated" | "0" | "4" => 4,
        _ => phase_str.parse::<i64>().unwrap_or(1),
    };
    ctx.set_var("boss_phase", Value::Int(phase_val));
    ctx.record_step(call.node_id, call.node_name, format!("Phase du Boss basculée vers {}", phase_str));
    Ok(())
}

fn h_boss_enrage(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    ctx.set_var("boss_enraged", Value::Bool(true));
    ctx.record_step(call.node_id, call.node_name, "Le Boss entre en mode OVERDRIVE ENRAGÉ !");
    Ok(())
}

fn h_set_boss_defeated(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    ctx.set_var("boss_defeated", Value::Bool(true));
    ctx.set_var("boss_phase", Value::Int(4));
    ctx.record_step(call.node_id, call.node_name, "Le Boss est terrassé ! Victoire totale !");
    Ok(())
}

fn h_trigger_boss_telegraph(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let atk = op.trim_start_matches("TriggerBossTelegraph:");
    ctx.set_var("boss_telegraph_attack", Value::String(atk.to_string()));
    ctx.record_step(call.node_id, call.node_name, format!("Télégraphie d'attaque armée : [{}]", atk));
    Ok(())
}

fn h_trigger_boss_roar(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    ctx.set_var("boss_roar_triggered", Value::Bool(true));
    ctx.record_step(call.node_id, call.node_name, "Grondement terrifiant du Boss !");
    Ok(())
}

fn h_stop_animation2_d(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    ctx.set_var("anim_request", Value::String("stop".into()));
    ctx.set_var("anim_request_entity", Value::String(String::new()));
    ctx.record_step(call.node_id, call.node_name, "Animation vectorielle arrêtée.");
    Ok(())
}

fn h_play_animation2_d_on(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let spec = op.trim_start_matches("PlayAnimation2DOn:");
    let mut it = spec.splitn(2, ':');
    let tag = it.next().unwrap_or("").to_string();
    let clip = it.next().unwrap_or("idle").to_string();
    ctx.set_var("anim_request", Value::String(clip.clone()));
    ctx.set_var("anim_request_entity", Value::String(tag.clone()));
    ctx.record_step(call.node_id, call.node_name, format!("Lecture de l'animation [{}] sur [{}]", clip, tag));
    Ok(())
}

fn h_play_animation2_d(ctx: &mut ExecutionContext, call: &NodeCall<'_>) -> Result<(), RuntimeError> {
    let op = call.type_id;
    let clip = op.trim_start_matches("PlayAnimation2D:").to_string();
    ctx.set_var("anim_request", Value::String(clip.clone()));
    ctx.set_var("anim_request_entity", Value::String(String::new()));
    ctx.record_step(call.node_id, call.node_name, format!("Lecture de l'animation [{}]", clip));
    Ok(())
}

/// Schémas des actions historiques migrées (catalogue unique).
pub(crate) fn schemas() -> Vec<NodeSchema> {
    vec![
        NodeSchema::action("AddVar", "Ajouter à une variable", Category::Progression, "Action historique. Forme préfixée : « AddVar:… ».")
            .with_prefix("AddVar:")
            .with_default_argument("score:1")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("AdvanceQuest", "Avancer une quête", Category::Progression, "Action historique. Forme préfixée : « AdvanceQuest:… ».")
            .with_prefix("AdvanceQuest:")
            .with_default_argument("quete")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("BonusPoints", "Bonus de points", Category::Progression, "Accorde un bonus majeur de points et déclenche la fanfare.")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("BoostSpeed", "Accélérer", Category::Movement, "Action historique migrée vers le registre.")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("BossEnrage", "Boss : mode enragé", Category::Progression, "Bascule l'indicateur de surcharge enragée et accélère les cycles d'attaque.")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("ClearAlarm", "Lever l'alerte", Category::World, "Action historique migrée vers le registre.")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("CloseDialogue", "Fermer le dialogue", Category::UserInterface, "Action historique migrée vers le registre.")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("CompleteQuest", "Terminer une quête", Category::Progression, "Action historique. Forme préfixée : « CompleteQuest:… ».")
            .with_prefix("CompleteQuest:")
            .with_default_argument("quete")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("DenyAccess", "Refuser l'accès", Category::World, "Signale le refus d'accès et empêche le passage.")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("DestroySelf", "Détruire l'objet", Category::World, "Supprime l'objet du monde de jeu.")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("EmitBurst", "Gerbe de particules", Category::World, "Action historique. Forme préfixée : « EmitBurst:… ».")
            .with_prefix("EmitBurst:")
            .with_default_argument("25")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("HealPlayer", "Soigner le joueur", Category::Combat, "Action historique. Forme préfixée : « HealPlayer:… ».")
            .with_prefix("HealPlayer:")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("IncrementCounter", "Incrémenter un compteur", Category::Progression, "Action historique. Forme préfixée : « IncrementCounter:… ».")
            .with_prefix("IncrementCounter:")
            .with_default_argument("compteur")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("LockAllDoors", "Verrouiller toutes les portes", Category::World, "Ferme immédiatement toute issue pour confiner l'intrus.")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("ModifySpeed", "Modifier la vitesse", Category::Movement, "Action historique. Forme préfixée : « ModifySpeed:… ».")
            .with_prefix("ModifySpeed:")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("NavChasePlayer", "Traquer le joueur (A*)", Category::ArtificialIntelligence, "Action historique migrée vers le registre.")
            .with_alias("ChasePlayer")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("NavGoTo", "Naviguer vers un point", Category::ArtificialIntelligence, "Action historique. Forme préfixée : « NavGoTo:… ».")
            .with_alias("NavigateTo")
            .with_prefix("NavGoTo:")
            .with_prefix("NavigateTo:")
            .with_default_argument("400,300")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("NavStop", "Arrêter la navigation", Category::ArtificialIntelligence, "Action historique migrée vers le registre.")
            .with_alias("StopNavigation")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("PlayAnimation2D", "Jouer une animation 2D", Category::World, "Action historique. Forme préfixée : « PlayAnimation2D:… ».")
            .with_prefix("PlayAnimation2D:")
            .with_default_argument("walk")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("PlayAnimation2DOn", "Jouer une animation sur une entité", Category::World, "Action historique. Forme préfixée : « PlayAnimation2DOn:… ».")
            .with_prefix("PlayAnimation2DOn:")
            .with_default_argument("player:walk")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("RemoveItem", "Retirer un objet", Category::Progression, "Action historique. Forme préfixée : « RemoveItem:… ».")
            .with_prefix("RemoveItem:")
            .with_default_argument("coin:1")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("ResetScore", "Réinitialiser le score", Category::Progression, "Action historique migrée vers le registre.")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("SelectHotbarSlot", "Choisir un emplacement de barre", Category::Progression, "Action historique. Forme préfixée : « SelectHotbarSlot:… ».")
            .with_prefix("SelectHotbarSlot:")
            .with_default_argument("0")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("SetBossDefeated", "Boss : vaincu", Category::Progression, "Définit le boss comme terrassé et déverrouille l'arène.")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("SetBossPhase", "Boss : changer de phase", Category::Combat, "Action historique. Forme préfixée : « SetBossPhase:… ».")
            .with_prefix("SetBossPhase:")
            .with_default_argument("2")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("SetQuest", "Définir l'étape d'une quête", Category::Progression, "Action historique. Forme préfixée : « SetQuest:… ».")
            .with_prefix("SetQuest:")
            .with_default_argument("quete:1")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("SetVar", "Fixer une variable", Category::Progression, "Action historique. Forme préfixée : « SetVar:… ».")
            .with_prefix("SetVar:")
            .with_default_argument("score:0")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("ShowDialogue", "Afficher un dialogue", Category::UserInterface, "Action historique. Forme préfixée : « ShowDialogue:… ».")
            .with_prefix("ShowDialogue:")
            .with_default_argument("PNJ:Bonjour")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("ShowDialogueChoice", "Afficher un dialogue à choix", Category::UserInterface, "Action historique. Forme préfixée : « ShowDialogueChoice:… ».")
            .with_prefix("ShowDialogueChoice:")
            .with_default_argument("PNJ:Bonjour:Oui:Non")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("SlowSpeed", "Ralentir", Category::Movement, "Action historique migrée vers le registre.")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("SpawnSparkles", "Étincelles", Category::World, "Projette des étincelles d'alerte rouge sur le joueur.")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("StopAnimation2D", "Arrêter l'animation 2D", Category::World, "Action historique migrée vers le registre.")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("SwarmScatter", "Meute : se disperser", Category::ArtificialIntelligence, "Action historique. Forme préfixée : « SwarmScatter:… ».")
            .with_prefix("SwarmScatter:")
            .with_default_argument("1,2.0")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("SwarmSetCohesion", "Meute : cohésion", Category::ArtificialIntelligence, "Action historique. Forme préfixée : « SwarmSetCohesion:… ».")
            .with_prefix("SwarmSetCohesion:")
            .with_default_argument("1,1.0")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("SwarmSetTarget", "Meute : cible", Category::ArtificialIntelligence, "Action historique. Forme préfixée : « SwarmSetTarget:… ».")
            .with_prefix("SwarmSetTarget:")
            .with_default_argument("1,400,300")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("SwarmTargetPlayer", "Meute : traquer le joueur", Category::ArtificialIntelligence, "Action historique. Forme préfixée : « SwarmTargetPlayer:… ».")
            .with_prefix("SwarmTargetPlayer:")
            .with_default_argument("1")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("TeleportPlayer", "Téléporter le joueur", Category::Movement, "Action historique. Forme préfixée : « TeleportPlayer:… ».")
            .with_prefix("TeleportPlayer:")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("TeleportTo", "Téléporter vers des coordonnées", Category::Movement, "Action historique. Forme préfixée : « TeleportTo:… ».")
            .with_prefix("TeleportTo:")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("ToggleDoor", "Ouvrir / fermer une porte", Category::World, "Bascule l'état ouvert/fermé de la porte.")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("ToggleEmitter", "Activer / désactiver l'émetteur", Category::World, "Action historique. Forme préfixée : « ToggleEmitter:… ».")
            .with_prefix("ToggleEmitter:")
            .with_default_argument("true")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("TriggerAlarm", "Déclencher l'alerte", Category::World, "Déclenche l'état d'alerte générale et la sirène.")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("TriggerBossRoar", "Boss : rugissement", Category::Combat, "Action historique migrée vers le registre.")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("TriggerBossTelegraph", "Boss : télégraphier une attaque", Category::Combat, "Action historique. Forme préfixée : « TriggerBossTelegraph:… ».")
            .with_prefix("TriggerBossTelegraph:")
            .with_default_argument("slam")
            .with_plugin(CORE_NODE_PACK),
        NodeSchema::action("UseItem", "Utiliser un objet", Category::Progression, "Action historique. Forme préfixée : « UseItem:… ».")
            .with_prefix("UseItem:")
            .with_default_argument("potion_heal")
            .with_plugin(CORE_NODE_PACK),
    ]
}
