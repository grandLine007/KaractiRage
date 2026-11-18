//! Vérification ponctuelle avec un GLB réel fourni par le propriétaire.
//! Ignoré par défaut : dépend d'un fichier hors dépôt (`KARACTI_GLB_REEL`).

use std::path::PathBuf;

#[test]
#[ignore]
fn glb_reel_importe_place_et_encode() {
    let chemin = PathBuf::from(std::env::var("KARACTI_GLB_REEL").expect("KARACTI_GLB_REEL"));
    let octets = std::fs::read(&chemin).expect("lecture du GLB");
    println!("GLB : {} octets", octets.len());

    let racine = std::env::temp_dir().join(format!("karacti_verif_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&racine);
    std::fs::create_dir_all(&racine).unwrap();

    let mut manifeste = karactirage_engine::assets_manifest::AssetsManifest::default();
    let importe = karactirage_engine::mesh_assets::enregistrer_mesh_importe(
        &racine,
        &mut manifeste,
        "chateau_gothique",
        &octets,
        Some("Inconnue"),
        "2026-11-18",
    )
    .expect("import accepté");
    println!(
        "importé : {} ({} octets sur disque)",
        importe.id,
        std::fs::metadata(racine.join(&importe.path)).unwrap().len()
    );
    assert_eq!(manifeste.entries.len(), 1);

    let mut engine = karactirage_engine::native_api::NativeEngine::init().expect("moteur");
    engine.game.assets_manifest = Some(manifeste);
    engine.game.camera3d = Some(karactirage_engine::subsystems::Camera3DData {
        position: [0.0, 6.0, 12.0],
        target: [0.0, 0.0, 0.0],
    });

    // Comme le Studio : ramener un château de ~29 unités à ~4 unités.
    let echelle = 4.0 / 29.693;
    let id = engine.place_mesh_asset(&importe.id, Some(echelle)).expect("placement accepté");
    let entite = engine.game.entities.iter().find(|e| e.id == id).expect("entité");
    println!("entité {} « {} » tag {}", entite.id, entite.name, entite.tag);

    let packet = karactirage_engine::render_packet::RenderPacket::decode(
        &engine.render_packet_frame(1.0 / 60.0),
    )
    .expect("paquet");
    let attendu = karactirage_engine::render_packet::asset_mesh_id(&importe.id);
    let batch = packet.batches.iter().find(|b| b.mesh_id == attendu).expect("batch de l'asset");
    println!("batch asset_id={} instances={}", batch.mesh_id, batch.instances.len());
    assert!(attendu >= 8);
    assert_eq!(batch.instances[0].entity_id, id as u32);

    // Cadrage : la caméra doit être hors du modèle (sinon on ne voit rien).
    let cam = engine.game.camera3d.as_ref().expect("caméra posée au placement");
    let distance = ((cam.position[2] - cam.target[2]).powi(2)
        + (cam.position[0] - cam.target[0]).powi(2))
    .sqrt();
    println!(
        "caméra z={:.2} cible z={:.2} distance={:.2}",
        cam.position[2], cam.target[2], distance
    );
    assert!(distance > 2.0, "caméra trop proche du modèle : {distance}");

    let _ = std::fs::remove_dir_all(&racine);
}

