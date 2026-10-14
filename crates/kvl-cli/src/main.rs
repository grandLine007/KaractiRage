use kvl_core::*;
use kvl_runtime::*;
use std::time::Instant;

fn main() {
    println!("=== KARACTIRAGE VISUAL LOGIC (KVL) - PROTOTYPE INITIAL ===");
    println!("Version: 0.1.0 (Rust Native)\n");

    // Construction du graphe de démonstration du document : "Ramasser une pièce"
    let mut graph = Graph::new(
        "coin_mechanic",
        "Mécanique de ramassage de pièce",
        "Détecte la collision, met à jour le score et détruit l'objet.",
    );

    graph.add_variable("score", Value::Int(0));

    let node_overlap = Node::new(
        "evt_pickup",
        "Quand le joueur touche la pièce",
        Category::Interactions,
        "Événement déclenché lors de la détection de collision avec le joueur.",
        NodeKind::Event {
            event_type: "OnPlayerOverlap".into(),
        },
    )
    .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

    let node_score = Node::new(
        "act_score",
        "Ajouter 1 au score",
        Category::Progression,
        "Incrémente le score de 1 point.",
        NodeKind::Action {
            action_type: "AddScore".into(),
        },
    )
    .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input))
    .with_output(Pin::new_trigger("out_trigger", "Exécution", PinDirection::Output));

    let node_destroy = Node::new(
        "act_destroy",
        "Faire disparaître la pièce",
        Category::World,
        "Retire l'entité de la scène.",
        NodeKind::Action {
            action_type: "DestroySelf".into(),
        },
    )
    .with_input(Pin::new_trigger("in_trigger", "Exécution", PinDirection::Input));

    graph.add_node(node_overlap);
    graph.add_node(node_score);
    graph.add_node(node_destroy);

    graph.add_connection(Connection::new(
        "conn_1",
        "evt_pickup",
        "out_trigger",
        "act_score",
        "in_trigger",
    ));

    graph.add_connection(Connection::new(
        "conn_2",
        "act_score",
        "out_trigger",
        "act_destroy",
        "in_trigger",
    ));

    println!("[1] Validation statique du modèle...");
    let diagnostics = Validator::validate(&graph);
    if diagnostics.is_empty() {
        println!("    ✓ Graphe 100% valide (aucune erreur structurelle).");
    } else {
        for diag in diagnostics {
            println!("    ! Diagnostic: {:?}", diag);
        }
    }

    println!("\n[2] Sérialisation JSON pour échange et sauvegarde (Agents IA / Éditeur)...");
    let json = graph.to_json().expect("Sérialisation réussie");
    println!("    Taille du graphe sérialisé : {} octets", json.len());

    println!("\n[3] Exécution dans le runtime KVL...");
    let mut ctx = Interpreter::init_context(&graph);
    println!("    Score initial : {:?}", ctx.get_var("score").unwrap());

    let start_time = Instant::now();
    let steps = Interpreter::trigger_event(&graph, &mut ctx, "OnPlayerOverlap")
        .expect("Exécution réussie");
    let duration = start_time.elapsed();

    println!("    Événement 'OnPlayerOverlap' résolu en : {:?}", duration);
    println!("    Nœuds exécutés : {}", steps);
    println!("    Nouveau score  : {:?}", ctx.get_var("score").unwrap());

    println!("\n[4] Journal d'exécution (traçabilité pas à pas) :");
    for step in &ctx.execution_log {
        println!("    → [{}] {} : {}", step.node_id, step.node_name, step.message);
    }

    println!("\n=== DÉMONSTRATION COMPLÈTE RÉUSSIE ===");
}
