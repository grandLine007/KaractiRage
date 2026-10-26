//! API native du moteur KaractiRage.
//!
//! Ce module encapsule une instance complète du moteur (`GameState` + `Canvas` +
//! `InputState`) derrière une API simple et **thread-safe** (la structure est
//! `Send`), afin d'être pilotée par le backend Tauri du Studio — un seul moteur,
//! une seule compilation.

use crate::camera::Camera3D;
use crate::entity::{Entity, MaterialRef, MeshRef, Renderable3D, Transform2D, Transform3D};
use crate::game::GameState;
use crate::input::InputState;
use crate::renderer::Canvas;
use glam::Vec3;
use winit::keyboard::KeyCode;

pub const CANVAS_WIDTH: usize = 1200;
pub const CANVAS_HEIGHT: usize = 800;

/// Pas de simulation fixe (120 Hz) pour un déterminisme indépendant du taux de
/// rafraîchissement de l'affichage.
pub const SIM_DT: f32 = 1.0 / 120.0;
/// Nombre maximal de sous-pas de simulation par frame (protection anti "spirale de la mort").
const MAX_SUBSTEPS: u32 = 4;

/// Ventilation du temps CPU d'une frame complète (en microsecondes).
/// Mesuré par `NativeEngine::render_frame` pour alimenter la télémétrie
/// et l'adaptation de qualité avec des valeurs réelles.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameStats {
    pub update_us: u64,
    pub render_us: u64,
    pub encode_us: u64,
    pub total_us: u64,
}

/// Instance embarquée du moteur, découplée de toute fenêtre OS.
pub struct NativeEngine {
    pub game: GameState,
    pub canvas: Canvas,
    pub input: InputState,
    /// Ventilation du temps de la dernière frame rendue.
    pub frame_stats: FrameStats,
    /// Accumulateur de temps pour le pas de simulation fixe.
    accumulator: f32,
    /// Temps simulé (s) pour la scène 3D de démonstration.
    packet_time: f32,
    /// Compteur de frames pour le `RenderPacket` (ordonnancement).
    packet_frame_id: u32,
    /// Caméra 3D de la vue (orbitale dans la démo).
    camera3d: Camera3D,
}

/// Résout le chemin de la scène par défaut, quel que soit le répertoire courant
/// (lancement depuis la racine du dépôt, ou depuis `apps/studio` via Tauri).
fn locate_scene() -> String {
    let candidates = [
        "assets/scenes/level_01.scene.json".to_string(),
        format!(
            "{}/../../assets/scenes/level_01.scene.json",
            env!("CARGO_MANIFEST_DIR")
        ),
    ];
    for c in candidates {
        if std::path::Path::new(&c).exists() {
            return c;
        }
    }
    "assets/scenes/level_01.scene.json".to_string()
}

fn web_key_to_keycode(key: u32) -> Option<KeyCode> {
    match key {
        0 => Some(KeyCode::ArrowLeft),
        1 => Some(KeyCode::ArrowRight),
        2 => Some(KeyCode::ArrowUp),
        3 => Some(KeyCode::ArrowDown),
        4 => Some(KeyCode::KeyW),
        5 => Some(KeyCode::KeyA),
        6 => Some(KeyCode::KeyS),
        7 => Some(KeyCode::KeyD),
        8 => Some(KeyCode::KeyZ),
        9 => Some(KeyCode::KeyQ),
        10 => Some(KeyCode::KeyE),
        11 => Some(KeyCode::Space),
        12 => Some(KeyCode::Tab),
        13 => Some(KeyCode::KeyN),
        14 => Some(KeyCode::KeyG),
        15 => Some(KeyCode::KeyM),
        16 => Some(KeyCode::F1),
        17 => Some(KeyCode::F2),
        18 => Some(KeyCode::F3),
        19 => Some(KeyCode::KeyR),
        20 => Some(KeyCode::Escape),
        21 => Some(KeyCode::ControlLeft),
        22 => Some(KeyCode::KeyV),
        23 => Some(KeyCode::KeyB),
        24 => Some(KeyCode::KeyX),
        25 => Some(KeyCode::KeyT),
        26 => Some(KeyCode::KeyP),
        27 => Some(KeyCode::KeyH),
        28 => Some(KeyCode::KeyU),
        29 => Some(KeyCode::F4),
        30 => Some(KeyCode::Digit1),
        31 => Some(KeyCode::Digit2),
        32 => Some(KeyCode::Digit3),
        33 => Some(KeyCode::Digit4),
        34 => Some(KeyCode::Digit5),
        35 => Some(KeyCode::KeyI),
        36 => Some(KeyCode::Enter),
        37 => Some(KeyCode::Delete),
        38 => Some(KeyCode::Backspace),
        39 => Some(KeyCode::KeyC),
        40 => Some(KeyCode::KeyY),
        _ => None,
    }
}

/// Échange les canaux rouge et bleu d'un pixel `0x00RRGGBB` (XRGB softbuffer)
/// pour produire un pixel `0xAABBGGRR` dont l'ordre mémoire little-endian est
/// exactement (R, G, B, A) — prêt à être copié vers un `<canvas>` HTML5.
#[inline]
fn swizzle_xrgb_to_rgba(px: u32) -> u32 {
    0xFF00_0000 | ((px & 0x0000_00FF) << 16) | (px & 0x0000_FF00) | ((px & 0x00FF_0000) >> 16)
}

// --- Helpers d'extraction des paramètres de recettes KVL guidées -------------

fn j_str(v: &serde_json::Value, key: &str, default: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or(default).to_string()
}

fn j_i64(v: &serde_json::Value, key: &str, default: i64) -> i64 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(default)
}

fn j_f32(v: &serde_json::Value, key: &str, default: f32) -> f32 {
    v.get(key).and_then(|x| x.as_f64()).map(|f| f as f32).unwrap_or(default)
}

fn j_bool(v: &serde_json::Value, key: &str, default: bool) -> bool {
    v.get(key).and_then(|x| x.as_bool()).unwrap_or(default)
}

fn j_opt_str(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(|s| s.to_string())
}

/// Produit un résumé en français lisible du graphe (événement → actions).
fn summarize_graph(graph: &kvl_core::Graph) -> String {
    let mut events: Vec<&kvl_core::Node> = Vec::new();
    let mut actions: Vec<&kvl_core::Node> = Vec::new();
    for node in graph.nodes.values() {
        match &node.kind {
            kvl_core::NodeKind::Event { .. } => events.push(node),
            kvl_core::NodeKind::Action { .. } => actions.push(node),
            _ => {}
        }
    }
    let mut parts: Vec<String> = Vec::new();
    for e in events {
        parts.push(format!("Quand « {} »", e.name));
    }
    for a in actions {
        parts.push(format!("→ {}", a.name));
    }
    if parts.is_empty() {
        parts.push(graph.description.clone());
    }
    parts.join(" ")
}

impl NativeEngine {
    /// Initialise une instance du moteur avec la scène par défaut.
    pub fn init() -> Result<Self, String> {
        let path = locate_scene();
        let mut game = GameState::load_from_scene(&path)
            .map_err(|e| format!("Impossible de charger la scène '{}': {}", path, e))?;
        game.editor.embedded_mode = true;
        game.editor.show_welcome_modal = false;
        game.editor.show_help_modal = false;

        let canvas = Canvas::new(CANVAS_WIDTH, CANVAS_HEIGHT);
        crate::renderer::warmup_glyph_cache();
        let input = InputState::new();

        Ok(Self {
            game,
            canvas,
            input,
            frame_stats: FrameStats::default(),
            accumulator: 0.0,
            packet_time: 0.0,
            packet_frame_id: 0,
            camera3d: Camera3D::perspective(Vec3::new(0.0, 7.0, 16.0), Vec3::ZERO, 60.0, 0.1, 200.0, 1.5),
        })
    }

    pub fn canvas_width(&self) -> u32 {
        CANVAS_WIDTH as u32
    }

    pub fn canvas_height(&self) -> u32 {
        CANVAS_HEIGHT as u32
    }

    /// Avance la simulation par pas fixes (120 Hz), génère le rendu logiciel,
    /// encode le framebuffer en octets RGBA, puis renseigne la télémétrie complète.
    pub fn render_frame(&mut self, dt: f32) -> Vec<u8> {
        // Clamp défensif : un très long gel d'UI ne doit pas provoquer une
        // avalanche de sous-pas (spirale de la mort).
        self.accumulator += dt.clamp(0.0, 0.25);

        let t0 = std::time::Instant::now();
        let mut steps = 0u32;
        while self.accumulator >= SIM_DT && steps < MAX_SUBSTEPS {
            self.game.update(SIM_DT, &self.input);
            self.accumulator -= SIM_DT;
            steps += 1;
            // Les entrées transitoires (just_pressed / click) ne doivent être
            // consommées qu'une seule fois par frame, pas à chaque sous-pas.
            if steps == 1 {
                self.input.clear_transient();
            }
        }
        let t1 = std::time::Instant::now();
        self.game.render(&mut self.canvas);
        let t2 = std::time::Instant::now();
        // Sécurité : garantit que les transitoires sont purgées même sans sous-pas.
        self.input.clear_transient();
        let bytes = self.frame_bytes();
        let t3 = std::time::Instant::now();

        let update_us = (t1 - t0).as_micros() as u64;
        let render_us = (t2 - t1).as_micros() as u64;
        let encode_us = (t3 - t2).as_micros() as u64;
        let total_us = update_us + render_us + encode_us;
        self.frame_stats = FrameStats {
            update_us,
            render_us,
            encode_us,
            total_us,
        };
        self.game.finish_frame_telemetry(total_us);
        bytes
    }

    /// Nombre d'entités 3D présentes dans la scène courante.
    fn count_3d_entities(&self) -> usize {
        self.game
            .entities
            .iter()
            .filter(|e| e.transform3d.is_some() && e.renderable3d.is_some())
            .count()
    }

    /// Prochain identifiant d'entité disponible (max + 1).
    fn next_entity_id(&self) -> usize {
        self.game.entities.iter().map(|e| e.id).max().unwrap_or(0) + 1
    }

    /// Spawne une entité 3D dans la scène courante (`game.entities`). Elle est invisible
    /// pour le pipeline 2D (aucun composant 2D), mais persiste dans le projet.
    pub fn spawn_3d_entity(
        &mut self,
        mesh: MeshRef,
        material: MaterialRef,
        position: Vec3,
        scale: Vec3,
        rotation_euler_deg: Vec3,
    ) -> usize {
        let id = self.next_entity_id();
        let mut e = Entity::new(
            id,
            format!("3d_{id}"),
            "3d".to_string(),
            Transform2D::new(position.x, position.z, 1.0, 1.0),
        );
        e.transform3d = Some(Transform3D::from_euler_deg(position, rotation_euler_deg, scale));
        e.renderable3d = Some(Renderable3D {
            mesh,
            material,
            cast_shadow: true,
            receive_shadow: true,
        });
        self.game.entities.push(e);
        id
    }

    /// Spawne une entité 3D par nom de preset (cube, sphere, pillar, rock, platform, ground).
    pub fn spawn_3d_preset(&mut self, preset: &str, x: f32, y: f32, z: f32) -> usize {
        let (mesh, material, scale, rotation) = match preset {
            "sphere" => (
                MeshRef::Sphere,
                MaterialRef::Solid { color: 0xFF38BDF8, roughness: 0.3, metalness: 0.1 },
                Vec3::splat(1.0),
                Vec3::ZERO,
            ),
            "pillar" => (
                MeshRef::Asset { id: "pillar".into(), node: None },
                MaterialRef::Solid { color: 0xFFFFFFFF, roughness: 0.4, metalness: 0.0 },
                Vec3::ONE,
                Vec3::ZERO,
            ),
            "rock" => (
                MeshRef::Asset { id: "rock".into(), node: None },
                MaterialRef::Solid { color: 0xFFFFFFFF, roughness: 0.4, metalness: 0.0 },
                Vec3::ONE,
                Vec3::ZERO,
            ),
            "platform" => (
                MeshRef::Asset { id: "platform".into(), node: None },
                MaterialRef::Solid { color: 0xFFFFFFFF, roughness: 0.4, metalness: 0.0 },
                Vec3::ONE,
                Vec3::ZERO,
            ),
            "ground" => (
                MeshRef::Plane,
                MaterialRef::Solid { color: 0xFF1B2130, roughness: 0.9, metalness: 0.0 },
                Vec3::new(40.0, 40.0, 1.0),
                Vec3::new(-90.0, 0.0, 0.0),
            ),
            _ => (
                MeshRef::Cube,
                MaterialRef::Solid { color: 0xFF38BDF8, roughness: 0.4, metalness: 0.1 },
                Vec3::splat(0.9),
                Vec3::ZERO,
            ),
        };
        self.spawn_3d_entity(mesh, material, Vec3::new(x, y, z), scale, rotation)
    }

    /// Déplace une entité 3D sur le plan XZ (préserve Y, rotation et échelle).
    /// Retourne `false` si l'entité n'existe pas ou n'est pas une entité 3D.
    pub fn move_entity_3d(&mut self, id: usize, x: f32, z: f32) -> bool {
        if let Some(ent) = self.game.entities.iter_mut().find(|e| e.id == id) {
            if let Some(ref mut t3d) = ent.transform3d {
                t3d.position.x = x;
                t3d.position.z = z;
                // Miroir de la projection 2D (hiérarchie / sélection).
                ent.transform.x = x;
                ent.transform.y = z;
                return true;
            }
        }
        false
    }

    /// Peuple une scène 3D par défaut (sol + grille de cubes + pilliers du manifest).
    pub fn populate_default_3d_scene(&mut self) {
        // Sol (plan aplati).
        self.spawn_3d_entity(
            MeshRef::Plane,
            MaterialRef::Solid { color: 0xFF1B2130, roughness: 0.9, metalness: 0.0 },
            Vec3::ZERO,
            Vec3::new(40.0, 40.0, 1.0),
            Vec3::new(-90.0, 0.0, 0.0),
        );
        // Grille 5×5 de cubes teintés.
        for i in 0..5 {
            for j in 0..5 {
                let x = (i as f32 - 2.0) * 1.6;
                let z = (j as f32 - 2.0) * 1.6;
                let hue = 0.5 + (i as f32 / 5.0) * 0.9;
                self.spawn_3d_entity(
                    MeshRef::Cube,
                    MaterialRef::Solid {
                        color: crate::render_packet::hsv_to_argb(hue, 0.7, 0.55),
                        roughness: 0.4,
                        metalness: 0.1,
                    },
                    Vec3::new(x, 0.5, z),
                    Vec3::splat(0.9),
                    Vec3::ZERO,
                );
            }
        }
        // 4 pilliers (asset manifest).
        for (px, pz) in [(-6.0, -6.0), (6.0, -6.0), (-6.0, 6.0), (6.0, 6.0)] {
            self.spawn_3d_entity(
                MeshRef::Asset { id: "pillar".into(), node: None },
                MaterialRef::Solid { color: 0xFFFFFFFF, roughness: 0.4, metalness: 0.0 },
                Vec3::new(px, 0.0, pz),
                Vec3::ONE,
                Vec3::ZERO,
            );
        }
    }

    /// Peuple une scène 3D thématique pour un archétype (démo 3D « en 1 clic »).
    /// Retourne le nombre d'entités 3D résultant. `key` accepte les mêmes alias que
    /// `default_game_for_archetype` (rpg, platformer, shmup, sokoban, boss).
    pub fn populate_archetype_3d_scene(&mut self, key: &str) -> Result<usize, String> {
        let canonical = match key {
            "rpg" | "rpg_dungeon" | "dungeon" => "rpg",
            "platformer" => "platformer",
            "shmup" | "shootemup" | "space" => "shmup",
            "sokoban" | "puzzle_sokoban" | "puzzle" => "sokoban",
            "boss" | "boss_arena" | "titan" => "boss",
            _ => return Err(format!("Archétype inconnu : '{}'", key)),
        };

        // Retire les entités 3D existantes (les entités 2D restent intactes).
        self.game.entities.retain(|e| e.transform3d.is_none());

        // Sol commun (plan aplati).
        self.spawn_3d_entity(
            MeshRef::Plane,
            MaterialRef::Solid { color: 0xFF1B2130, roughness: 0.9, metalness: 0.0 },
            Vec3::ZERO,
            Vec3::new(40.0, 40.0, 1.0),
            Vec3::new(-90.0, 0.0, 0.0),
        );

        match canonical {
            "rpg" => {
                for i in 0..6 {
                    for j in 0..6 {
                        if i == 0 || i == 5 || j == 0 || j == 5 {
                            self.spawn_3d_entity(
                                MeshRef::Asset { id: "pillar".into(), node: None },
                                MaterialRef::Solid { color: 0xFFFFFFFF, roughness: 0.4, metalness: 0.0 },
                                Vec3::new((i as f32 - 2.5) * 2.4, 0.0, (j as f32 - 2.5) * 2.4),
                                Vec3::ONE,
                                Vec3::ZERO,
                            );
                        }
                    }
                }
                for (i, (cx, cz)) in [(-1.4, -1.4), (1.4, 1.4), (-1.4, 1.4)].iter().enumerate() {
                    self.spawn_3d_entity(
                        MeshRef::Cube,
                        MaterialRef::Solid { color: 0xFFF59E0B, roughness: 0.25, metalness: 0.6 },
                        Vec3::new(*cx, 0.45, *cz),
                        Vec3::new(0.8, 0.8, 0.8),
                        Vec3::new(0.0, (i as f32) * 30.0, 0.0),
                    );
                }
                for (tx, tz) in [(-2.4, 0.0), (2.4, 0.0)] {
                    self.spawn_3d_entity(
                        MeshRef::Sphere,
                        MaterialRef::Solid { color: 0xFFFFB84D, roughness: 0.3, metalness: 0.0 },
                        Vec3::new(tx, 1.6, tz),
                        Vec3::splat(0.25),
                        Vec3::ZERO,
                    );
                }
            },
            "platformer" => {
                for (i, (x, y, z)) in [(-4.0, 1.2, 0.0), (-1.5, 2.0, 1.0), (1.5, 2.8, -1.0), (4.0, 1.6, 0.0)].iter().enumerate() {
                    self.spawn_3d_entity(
                        MeshRef::Asset { id: "platform".into(), node: None },
                        MaterialRef::Solid { color: 0xFF8B5CF6, roughness: 0.4, metalness: 0.1 },
                        Vec3::new(*x, *y, *z),
                        Vec3::ONE,
                        Vec3::new(0.0, (i as f32) * 45.0, 0.0),
                    );
                }
                for (cx, cy, cz) in [(-4.0, 2.0, 0.0), (-1.5, 2.8, 1.0), (1.5, 3.6, -1.0), (4.0, 2.4, 0.0)] {
                    self.spawn_3d_entity(
                        MeshRef::Sphere,
                        MaterialRef::Solid { color: 0xFFFBBF24, roughness: 0.2, metalness: 0.7 },
                        Vec3::new(cx, cy, cz),
                        Vec3::splat(0.3),
                        Vec3::ZERO,
                    );
                }
                self.spawn_3d_entity(
                    MeshRef::Capsule,
                    MaterialRef::Solid { color: 0xFF8B5CF6, roughness: 0.4, metalness: 0.0 },
                    Vec3::new(0.0, 0.9, 0.0),
                    Vec3::new(0.6, 0.8, 0.6),
                    Vec3::ZERO,
                );
            },
            "shmup" => {
                for i in 0..10 {
                    let x = ((i as f32) * 7.0) % 12.0 - 6.0;
                    let z = ((i as f32) * 5.0) % 12.0 - 6.0;
                    let s = 0.4 + ((i as f32) * 0.37) % 0.9;
                    self.spawn_3d_entity(
                        MeshRef::Asset { id: "rock".into(), node: None },
                        MaterialRef::Solid { color: 0xFF78716C, roughness: 0.8, metalness: 0.0 },
                        Vec3::new(x, s * 0.6, z),
                        Vec3::splat(s),
                        Vec3::new((i as f32) * 33.0, (i as f32) * 17.0, 0.0),
                    );
                }
                for i in 0..4 {
                    let x = ((i as f32) * 3.0) % 8.0 - 4.0;
                    let z = ((i as f32) * 2.0) % 8.0 - 4.0;
                    self.spawn_3d_entity(
                        MeshRef::Capsule,
                        MaterialRef::Solid { color: 0xFFEC4899, roughness: 0.3, metalness: 0.5 },
                        Vec3::new(x, 1.2, z),
                        Vec3::new(0.5, 1.0, 0.5),
                        Vec3::new(0.0, 0.0, 90.0),
                    );
                }
                self.spawn_3d_entity(
                    MeshRef::Capsule,
                    MaterialRef::Solid { color: 0xFF38BDF8, roughness: 0.2, metalness: 0.4 },
                    Vec3::new(0.0, 1.6, -2.0),
                    Vec3::new(0.7, 1.2, 0.7),
                    Vec3::new(90.0, 0.0, 0.0),
                );
            },
            "sokoban" => {
                for (i, (cx, cz)) in [(-2.0, -2.0), (0.0, -2.0), (2.0, 0.0), (-1.0, 1.0)].iter().enumerate() {
                    self.spawn_3d_entity(
                        MeshRef::Cube,
                        MaterialRef::Solid { color: 0xFFF59E0B, roughness: 0.35, metalness: 0.2 },
                        Vec3::new(*cx, 0.5, *cz),
                        Vec3::splat(0.9),
                        Vec3::new(0.0, (i as f32) * 15.0, 0.0),
                    );
                }
                for (px, pz) in [(-2.0, 1.0), (2.0, -2.0)] {
                    self.spawn_3d_entity(
                        MeshRef::Cube,
                        MaterialRef::Solid { color: 0xFF38BDF8, roughness: 0.4, metalness: 0.1 },
                        Vec3::new(px, 0.1, pz),
                        Vec3::new(1.0, 0.2, 1.0),
                        Vec3::ZERO,
                    );
                }
                for (ox, oz) in [(-1.0, 0.0), (1.0, 1.0)] {
                    self.spawn_3d_entity(
                        MeshRef::Sphere,
                        MaterialRef::Solid { color: 0xFF34D399, roughness: 0.2, metalness: 0.6 },
                        Vec3::new(ox, 1.0, oz),
                        Vec3::splat(0.3),
                        Vec3::ZERO,
                    );
                }
            },
            _ => {
                for i in 0..10 {
                    let a = (i as f32 / 10.0) * std::f32::consts::TAU;
                    self.spawn_3d_entity(
                        MeshRef::Asset { id: "pillar".into(), node: None },
                        MaterialRef::Solid { color: 0xFFFFFFFF, roughness: 0.4, metalness: 0.0 },
                        Vec3::new(a.cos() * 7.0, 0.0, a.sin() * 7.0),
                        Vec3::ONE,
                        Vec3::ZERO,
                    );
                }
                for i in 0..6 {
                    let a = (i as f32 / 6.0) * std::f32::consts::TAU;
                    self.spawn_3d_entity(
                        MeshRef::Asset { id: "rock".into(), node: None },
                        MaterialRef::Solid { color: 0xFF78716C, roughness: 0.8, metalness: 0.0 },
                        Vec3::new(a.cos() * 4.0, 0.0, a.sin() * 4.0),
                        Vec3::splat(0.8),
                        Vec3::ZERO,
                    );
                }
                self.spawn_3d_entity(
                    MeshRef::Capsule,
                    MaterialRef::Solid { color: 0xFFEF4444, roughness: 0.35, metalness: 0.3 },
                    Vec3::new(0.0, 1.6, 0.0),
                    Vec3::new(1.4, 1.6, 1.4),
                    Vec3::ZERO,
                );
            },
        }

        Ok(self.game.entities.iter().filter(|e| e.transform3d.is_some()).count())
    }

    /// Construit le `RenderPacket` depuis les entités 3D réelles de la scène.
    fn build_3d_packet(&self) -> crate::render_packet::RenderPacket {
        let manifest = crate::mesh_assets::MeshManifest::builtin();
        let refs: Vec<&Entity> = self
            .game
            .entities
            .iter()
            .filter(|e| e.transform3d.is_some() && e.renderable3d.is_some())
            .collect();
        let mut packet = crate::render_packet::packet_from_entities(
            &manifest,
            &self.camera3d,
            &refs,
            self.packet_frame_id,
        );
        packet.ambient = [0.4, 0.4, 0.4];
        packet.lights = vec![crate::render_packet::LightPacket {
            kind: crate::render_packet::LightKind::Directional,
            color: [1.0, 1.0, 1.0],
            intensity: 2.2,
            pos: [8.0, 14.0, 6.0],
            dir: [-0.5, -0.8, -0.4],
            range: 0.0,
            spot_cos_cutoff: 0.0,
        }];
        packet
    }

    /// Construit et encode le `RenderPacket` 3D depuis les entités réelles de la scène.
    /// La scène est auto-peuplée au premier appel ; la caméra orbite lentement.
    pub fn render_packet_frame(&mut self, dt: f32) -> Vec<u8> {
        self.packet_time += dt.clamp(0.0, 0.25);
        if self.count_3d_entities() == 0 {
            self.populate_default_3d_scene();
        }
        // Orbite lente de la caméra pour une vue dynamique (démo).
        let angle = self.packet_time * 0.15;
        let radius = 16.0;
        let height = 7.0;
        self.camera3d.position = Vec3::new(radius * angle.sin(), height, radius * angle.cos());
        self.camera3d.target = Vec3::ZERO;
        self.camera3d.aspect = 1.5;

        let packet = self.build_3d_packet();
        self.packet_frame_id = self.packet_frame_id.wrapping_add(1);
        packet.encode()
    }

    /// Encode le framebuffer courant en octets RGBA (R, G, B, A par pixel),
    /// en une seule passe : échange R/B + écriture directe des 4 octets du pixel.
    pub fn frame_bytes(&self) -> Vec<u8> {
        let n = self.canvas.buffer.len();
        if n == 0 {
            return Vec::new();
        }

        let mut out = Vec::with_capacity(n * 4);
        // Le tampon alloué par `Vec<u8>` est aligné pour `u32` (≥ 4 octets) ;
        // on le réinterprète en tranche de `u32` pour écrire 4 octets par store.
        let out_u32: &mut [u32] =
            unsafe { std::slice::from_raw_parts_mut(out.as_mut_ptr() as *mut u32, n) };
        for (i, px) in self.canvas.buffer.iter().enumerate() {
            out_u32[i] = swizzle_xrgb_to_rgba(*px);
        }
        // Safety: les `n` éléments u32 (soit `n * 4` octets) ont tous été écrits.
        unsafe { out.set_len(n * 4) };
        out
    }

    // --- Entrées ---------------------------------------------------------------

    pub fn on_key_down(&mut self, key: u32) {
        if let Some(kc) = web_key_to_keycode(key) {
            self.input.on_key_down(kc);
        }
    }

    pub fn on_key_up(&mut self, key: u32) {
        if let Some(kc) = web_key_to_keycode(key) {
            self.input.on_key_up(kc);
        }
    }

    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        self.input.on_mouse_move(x, y);
    }

    pub fn on_mouse_down(&mut self, button: u32) {
        if button == 0 {
            self.input.on_mouse_down();
        } else if button == 2 {
            self.input.on_mouse_right_click();
        }
    }

    pub fn on_mouse_up(&mut self, button: u32) {
        if button == 0 {
            self.input.on_mouse_up();
        }
    }

    // --- Télémétrie -----------------------------------------------------------

    pub fn get_score(&self) -> i32 {
        self.game
            .kvl_context
            .get_var("score")
            .map_or(0, |v| match v {
                kvl_core::Value::Int(s) => *s as i32,
                _ => 0,
            })
    }

    pub fn get_fps(&self) -> f32 {
        self.game.telemetry.current_fps
    }

    pub fn get_cpu_us(&self) -> u32 {
        self.game.telemetry.avg_frame_time_us as u32
    }

    pub fn get_spatial_queries(&self) -> u32 {
        self.game.spatial_grid.stats_queries as u32
    }

    pub fn get_status_message(&self) -> String {
        self.game.status_message.clone()
    }

    // --- Bascules -------------------------------------------------------------

    pub fn toggle_radar(&mut self) -> u32 {
        self.game.show_minimap = !self.game.show_minimap;
        if self.game.show_minimap {
            1
        } else {
            0
        }
    }

    pub fn toggle_spatial_grid(&mut self) -> u32 {
        if self.game.spatial_grid.toggle_debug() {
            1
        } else {
            0
        }
    }

    pub fn toggle_editor_mode(&mut self) -> u32 {
        self.game.editor.toggle_mode();
        match self.game.editor.mode {
            crate::editor::EditorMode::Edit => 1,
            crate::editor::EditorMode::Play => 0,
        }
    }

    pub fn set_embedded_mode(&mut self, enabled: bool) {
        self.game.editor.embedded_mode = enabled;
    }

    // --- Export / import ------------------------------------------------------

    pub fn export_standalone_html(&self, title: &str, author: &str) -> String {
        crate::exporter::StandaloneExporter::export_html(&self.game, title, author)
    }

    pub fn export_game_package(&self, title: &str, author: &str) -> String {
        crate::exporter::StandaloneExporter::export_package_json(&self.game, title, author)
            .unwrap_or_else(|_| "{}".to_string())
    }

    pub fn import_game_package(&mut self, package_json: &str) -> i32 {
        let package = match crate::exporter::KaractiGamePackage::from_json(package_json) {
            Ok(p) => p,
            Err(_) => return -1,
        };
        crate::exporter::StandaloneExporter::import_package(&mut self.game, &package);
        0
    }

    // --- Recettes --------------------------------------------------------------

    pub fn validate_recipe(recipe_json: &str) -> String {
        let mut recipe = match crate::recipe::KaractiGameRecipe::from_json(recipe_json) {
            Ok(r) => r,
            Err(e) => {
                return format!("{{\"is_valid\":false,\"conflicts\":[\"JSON invalide: {}\"]}}", e)
            }
        };
        let report = crate::recipe::DependencyResolver::resolve(&mut recipe);
        serde_json::to_string(&report).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn apply_recipe(&mut self, recipe_json: &str) -> i32 {
        let mut recipe = match crate::recipe::KaractiGameRecipe::from_json(recipe_json) {
            Ok(r) => r,
            Err(_) => return -1,
        };
        let report = crate::recipe::DependencyResolver::resolve(&mut recipe);
        if !report.is_valid {
            return -2;
        }
        let (scene, kvl_graphs) = crate::recipe::RecipeProjectGenerator::generate(&recipe);
        self.game.scene_name = scene.name;
        self.game.entities = scene.entities;
        self.game.kvl_graphs = kvl_graphs;
        self.game.editor.selected = crate::editor::SelectedEntity::None;
        self.game.editor.mode = crate::editor::EditorMode::Play;
        self.game
            .set_status(&format!("Recette '{}' instanciée avec succès !", recipe.title));
        0
    }

    /// Lance le jeu de référence d'un archétype (commande Tauri `engine_launch_archetype`).
    pub fn launch_default_game(&mut self, key: &str) -> Result<String, String> {
        let name = self.game.launch_default_game(key)?;
        self.game.editor.embedded_mode = true;
        let mut registry = crate::project_registry::ProjectRegistry::open();
        registry.record_recent(&self.game.scene_path);
        let _ = registry.save();
        Ok(name)
    }

    // --- IA --------------------------------------------------------------------

    pub fn apply_ai_payload(&mut self, payload_json: &str, prompt: &str) -> i32 {
        let payload: crate::ai_client::AiGeneratedGamePayload =
            match serde_json::from_str(payload_json) {
                Ok(p) => p,
                Err(_) => return -1,
            };
        self.game.apply_ai_payload(payload, prompt);
        0
    }

    pub fn accept_ai_patch(&mut self) -> i32 {
        self.game.accept_ai_patch();
        0
    }

    pub fn reject_ai_patch(&mut self) -> i32 {
        self.game.reject_ai_patch();
        0
    }

    pub fn has_pending_ai_patch(&self) -> i32 {
        if self.game.editor.pending_ai_patch.is_some() {
            1
        } else {
            0
        }
    }

    // --- Outils Studio ---------------------------------------------------------

    pub fn set_active_tool(&mut self, tool: u32) -> u32 {
        let tool_enum = match tool {
            0 => crate::editor::StudioTool::Select,
            1 => crate::editor::StudioTool::Brush,
            2 => crate::editor::StudioTool::Eraser,
            _ => crate::editor::StudioTool::Select,
        };
        self.game.editor.set_tool(tool_enum);
        tool
    }

    pub fn get_active_tool(&self) -> u32 {
        match self.game.editor.active_tool {
            crate::editor::StudioTool::Select => 0,
            crate::editor::StudioTool::Brush => 1,
            crate::editor::StudioTool::Eraser => 2,
        }
    }

    pub fn cycle_brush_tile(&mut self) -> String {
        self.game.editor.cycle_brush_tile().label().to_string()
    }

    pub fn get_active_brush_tile(&self) -> String {
        self.game.editor.brush_tile.label().to_string()
    }

    // --- Navigation / interaction ----------------------------------------------

    pub fn move_player_to(&mut self, target_x: f32, target_y: f32) -> bool {
        if let Some(p_idx) = self.game.player_index() {
            let mut nav = crate::navigation::NavAgent::go_to(260.0, target_x, target_y);
            nav.reach_distance = 18.0;
            nav.avoid_hazards = true;
            self.game.entities[p_idx].nav_agent = Some(nav);
            self.game.status_message = format!(
                "Navigation A* vers ({:.0}, {:.0}) enclenchée.",
                target_x, target_y
            );
            return true;
        }
        false
    }

    pub fn get_player_pos(&self) -> (f32, f32) {
        if let Some(p_idx) = self.game.player_index() {
            (
                self.game.entities[p_idx].transform.x,
                self.game.entities[p_idx].transform.y,
            )
        } else {
            (0.0, 0.0)
        }
    }

    pub fn interact(&mut self) -> bool {
        self.input.on_key_down(KeyCode::KeyE);
        true
    }

    // --- Projets ---------------------------------------------------------------

    pub fn new_blank_project(&mut self, title: &str, genre_str: &str) -> i32 {
        let genre = match genre_str {
            "platformer" => crate::recipe::GameGenre::Platformer,
            "shoot_em_up" | "shmup" => crate::recipe::GameGenre::ShootEmUp,
            "puzzle" | "sokoban" => crate::recipe::GameGenre::Puzzle,
            "combat" | "boss" => crate::recipe::GameGenre::Combat,
            _ => crate::recipe::GameGenre::Adventure,
        };
        self.game.new_blank_project(title, genre);
        0
    }

    pub fn load_project_json(&mut self, project_json: &str) -> i32 {
        let proj = match crate::project::KaractiProject::from_json(project_json) {
            Ok(p) => p,
            Err(_) => return -2,
        };
        self.game.load_project(proj);
        0
    }

    pub fn export_project_json(&self) -> String {
        self.game
            .export_current_project()
            .to_json()
            .unwrap_or_else(|_| "{}".to_string())
    }

    /// Ouvre un projet (`.karacti`) ou une scène de démo (`.scene.json`) depuis un
    /// chemin de fichier sur le disque, puis force le mode embarqué Studio.
    pub fn open_project_path(&mut self, path: &str) -> Result<i32, String> {
        if path.ends_with(".scene.json") {
            if !self.game.switch_scene(path) {
                return Err(format!("Impossible de charger la scène '{}'", path));
            }
        } else {
            let proj = crate::project::KaractiProject::load_from_file(path)
                .map_err(|e| format!("Impossible de charger le projet : {}", e))?;
            self.game.load_project(proj);
        }
        // Le Studio Tauri est toujours embarqué : pas de chrome softbuffer natif.
        self.game.editor.embedded_mode = true;
        self.game.editor.show_welcome_modal = false;
        self.game.editor.show_help_modal = false;
        Ok(0)
    }

    pub fn get_entities_summary_json(&self) -> String {
        self.game.get_entities_summary_json()
    }

    pub fn update_entity_transform(
        &mut self,
        id: usize,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        record_history: bool,
    ) -> bool {
        self.game
            .update_entity_transform_by_id(id, x, y, w, h, record_history)
    }

    pub fn delete_entity(&mut self, id: usize) -> bool {
        self.game.delete_entity_by_id(id)
    }

    /// Instantané complet de l'état de l'éditeur (entités + sélection + outil + tuile + undo/redo).
    pub fn editor_state_json(&self) -> String {
        self.game.editor_state_json()
    }

    pub fn select_entity(&mut self, id: usize) -> bool {
        self.game.select_entity_by_id(id)
    }

    pub fn get_selected_entity_id(&self) -> Option<usize> {
        self.game.selected_entity_id()
    }

    pub fn delete_selected_entity(&mut self) -> bool {
        self.game.remove_selected_entity()
    }

    pub fn duplicate_selected_entity(&mut self) -> Option<usize> {
        self.game.duplicate_selected_entity()
    }

    pub fn undo(&mut self) -> bool {
        self.game.undo()
    }

    pub fn redo(&mut self) -> bool {
        self.game.redo()
    }

    /// Règle le mode de l'éditeur (0 = Jeu, 1 = Studio) sans bascule aveugle.
    pub fn set_editor_mode(&mut self, mode: u32) -> u32 {
        self.game.editor.mode = if mode == 1 {
            crate::editor::EditorMode::Edit
        } else {
            crate::editor::EditorMode::Play
        };
        self.game.editor.last_painted_cell = None;
        self.game.editor.is_dragging = false;
        mode
    }

    /// Règle la tuile active du pinceau par identifiant (et bascule l'outil Pinceau).
    pub fn set_brush_tile_by_id(&mut self, tile_id: &str) -> String {
        use crate::editor::BrushTile;
        let tile = match tile_id {
            "wall" => BrushTile::Wall,
            "hazard" => BrushTile::Hazard,
            "collectible" => BrushTile::Collectible,
            "heart" => BrushTile::Heart,
            "crate" => BrushTile::Crate,
            "plate" => BrushTile::Plate,
            _ => return self.game.editor.brush_tile.label().to_string(),
        };
        self.game.editor.brush_tile = tile;
        self.game.editor.active_tool = crate::editor::StudioTool::Brush;
        tile.label().to_string()
    }

    /// Liste sérialisée des 6 tuiles de pinceau (id, libellés, couleur hexadécimale).
    pub fn get_brush_tiles_json(&self) -> String {
        use crate::editor::BrushTile;
        let tiles = [
            BrushTile::Wall,
            BrushTile::Hazard,
            BrushTile::Collectible,
            BrushTile::Heart,
            BrushTile::Crate,
            BrushTile::Plate,
        ];
        let v: Vec<serde_json::Value> = tiles
            .iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id(),
                    "label": t.label(),
                    "short": t.short_label(),
                    "color": format!("#{:06X}", t.color() & 0xFFFFFF),
                })
            })
            .collect();
        serde_json::to_string(&v).unwrap_or_else(|_| "[]".to_string())
    }

    /// Instancie un préfabriqué à une position écran (convertie en coordonnées monde).
    pub fn spawn_preset_at_screen(&mut self, preset: &str, sx: f32, sy: f32) -> usize {
        let (wx, wy) = self.game.camera.screen_to_world(sx, sy);
        self.spawn_entity_preset(preset, wx, wy)
    }

    /// Sauvegarde la scène ECS et les scripts KVL sur disque.
    pub fn save_scene(&mut self) -> bool {
        self.game.save_scene_and_scripts().is_ok()
    }

    // --- KVL Guidé -------------------------------------------------------------

    /// Liste sérialisée des graphes KVL chargés (path, nom, description, tailles).
    pub fn get_kvl_graphs_json(&self) -> String {
        let v: Vec<serde_json::Value> = self
            .game
            .kvl_graphs
            .iter()
            .map(|(path, g)| {
                serde_json::json!({
                    "path": path,
                    "id": g.id,
                    "name": g.name,
                    "description": g.description,
                    "node_count": g.nodes.len(),
                    "connection_count": g.connections.len(),
                })
            })
            .collect();
        serde_json::to_string(&v).unwrap_or_else(|_| "[]".to_string())
    }

    /// Renvoie le JSON complet d'un graphe KVL par son chemin, ou `null`.
    pub fn get_kvl_graph_json(&self, path: &str) -> String {
        self.game
            .kvl_graphs
            .get(path)
            .and_then(|g| g.to_json().ok())
            .unwrap_or_else(|| "null".to_string())
    }

    /// Valide un graphe KVL (JSON) et renvoie ses diagnostics sérialisés.
    pub fn validate_kvl_graph_json(&self, json: &str) -> String {
        let graph = match kvl_core::Graph::from_json(json) {
            Ok(g) => g,
            Err(e) => {
                return serde_json::json!([{
                    "severity": "error",
                    "message": format!("JSON invalide : {}", e),
                    "node_id": null,
                    "pin_id": null,
                }])
                .to_string()
            }
        };
        let diags = kvl_core::Validator::validate(&graph);
        let v: Vec<serde_json::Value> = diags
            .iter()
            .map(|d| {
                serde_json::json!({
                    "severity": match d.severity { kvl_core::ValidationSeverity::Error => "error", kvl_core::ValidationSeverity::Warning => "warning" },
                    "message": d.message,
                    "node_id": d.node_id,
                    "pin_id": d.pin_id,
                })
            })
            .collect();
        serde_json::to_string(&v).unwrap_or_else(|_| "[]".to_string())
    }

    /// Enregistre un graphe KVL (JSON) en mémoire et sur disque.
    pub fn save_kvl_graph(&mut self, path: &str, json: &str) -> bool {
        let graph = match kvl_core::Graph::from_json(json) {
            Ok(g) => g,
            Err(_) => return false,
        };
        self.game.kvl_graphs.insert(path.to_string(), graph.clone());
        self.persist_kvl_graph_to_disk(path, &graph);
        true
    }

    /// Attache un script KVL existant à une entité (par id) avec un événement de recouvrement.
    pub fn attach_script_to_entity(&mut self, entity_id: usize, script_path: &str) -> bool {
        if let Some(ent) = self.game.entities.iter_mut().find(|e| e.id == entity_id) {
            ent.script = Some(crate::entity::KVLScriptComponent::new(script_path).with_overlap("OnPlayerOverlap"));
            return true;
        }
        false
    }

    /// Résout l'index d'entité cible : `entity_id` passé, sinon la sélection courante.
    fn resolve_entity_index(&self, params: &serde_json::Value) -> Option<usize> {
        if let Some(id) = params.get("entity_id").and_then(|v| v.as_u64()) {
            if let Some(idx) = self.game.entities.iter().position(|e| e.id == id as usize) {
                return Some(idx);
            }
        }
        match self.game.editor.selected {
            crate::editor::SelectedEntity::Entity(idx) if idx < self.game.entities.len() => Some(idx),
            _ => None,
        }
    }

    fn persist_kvl_graph_to_disk(&self, path: &str, graph: &kvl_core::Graph) {
        if let Ok(json) = graph.to_json() {
            let abs = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path);
            if let Some(parent) = std::path::Path::new(&abs).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&abs, json);
        }
    }

    /// Construit un graphe KVL guidé, l'attache à l'entité sélectionnée (ou en
    /// crée une), le valide et le persiste. Renvoie une enveloppe JSON complète.
    pub fn build_guided_graph(&mut self, recipe: &str, params: serde_json::Value) -> String {
        use kvl_core::GuidedBuilder;

        let (graph, trigger_event, default_kind) = match recipe {
            "collectible" => {
                let intent = kvl_core::CollectibleIntent {
                    item_name: j_str(&params, "item_name", "Objet à ramasser"),
                    score_points: j_i64(&params, "score_points", 10),
                    destroy_on_pickup: j_bool(&params, "destroy_on_pickup", true),
                    play_sound: j_opt_str(&params, "play_sound"),
                };
                (GuidedBuilder::build_collectible(&intent), "OnPlayerOverlap", "collectible")
            }
            "hazard" => {
                let intent = kvl_core::HazardIntent {
                    hazard_name: j_str(&params, "hazard_name", "Zone Dangereuse"),
                    damage_amount: j_i64(&params, "damage_amount", 20),
                    play_sound: j_opt_str(&params, "play_sound"),
                };
                (GuidedBuilder::build_hazard(&intent), "OnPlayerOverlap", "hazard")
            }
            "teleport" => {
                let intent = kvl_core::TeleportIntent {
                    portal_name: j_str(&params, "portal_name", "Portail"),
                    target_x: j_f32(&params, "target_x", 400.0),
                    target_y: j_f32(&params, "target_y", 300.0),
                    play_sound: j_opt_str(&params, "play_sound"),
                };
                (GuidedBuilder::build_teleport(&intent), "OnPlayerOverlap", "portal")
            }
            "switch" => {
                let intent = kvl_core::SwitchPlateIntent {
                    switch_name: j_str(&params, "switch_name", "Dalle Pression"),
                    door_var_to_open: j_str(&params, "door_var_to_open", "door_secret"),
                    play_sound: j_opt_str(&params, "play_sound"),
                };
                (GuidedBuilder::build_switch_plate(&intent), "OnPressurePlate", "plate")
            }
            "chest" => {
                let intent = kvl_core::ChestIntent {
                    chest_name: j_str(&params, "chest_name", "Coffre"),
                    required_key: j_opt_str(&params, "required_key"),
                    reward_score: j_i64(&params, "reward_score", 100),
                    reward_item: j_opt_str(&params, "reward_item"),
                    play_sound: j_opt_str(&params, "play_sound"),
                };
                (GuidedBuilder::build_chest(&intent), "OnDoorInteract", "crate")
            }
            "door" => {
                let trigger = match params.get("trigger").and_then(|x| x.as_str()).unwrap_or("proximity") {
                    "key" => kvl_core::DoorTriggerType::OnKeyPress(j_str(&params, "key", "E")),
                    _ => kvl_core::DoorTriggerType::OnProximity,
                };
                let condition = match params.get("condition").and_then(|x| x.as_str()).unwrap_or("none") {
                    "score" => kvl_core::DoorConditionType::ScoreRequired(j_i64(&params, "score", 10)),
                    "item" => kvl_core::DoorConditionType::ItemRequired(j_str(&params, "item", "key_gold")),
                    _ => kvl_core::DoorConditionType::None,
                };
                let intent = kvl_core::DoorIntent {
                    door_name: j_str(&params, "door_name", "Porte"),
                    trigger,
                    condition,
                    toggle: j_bool(&params, "toggle", false),
                    play_sound: j_opt_str(&params, "play_sound"),
                };
                (GuidedBuilder::build_door(&intent), "OnDoorInteract", "door")
            }
            _ => {
                return serde_json::json!({ "ok": false, "error": format!("Recette '{}' inconnue.", recipe) }).to_string()
            }
        };

        let diagnostics = kvl_core::Validator::validate(&graph);
        let graph_json = graph.to_json().unwrap_or_else(|_| "{}".to_string());
        let summary = summarize_graph(&graph);

        let script_path = format!(
            "assets/scripts/guided_{}_{}.kvl.json",
            recipe,
            self.game.entities.len() + 1
        );
        self.game.kvl_graphs.insert(script_path.clone(), graph.clone());
        self.persist_kvl_graph_to_disk(&script_path, &graph);

        let mut attached = false;
        let mut target_entity: Option<String> = None;
        if let Some(idx) = self.resolve_entity_index(&params) {
            if let Some(ent) = self.game.entities.get_mut(idx) {
                ent.script = Some(crate::entity::KVLScriptComponent::new(&script_path).with_overlap(trigger_event));
                attached = true;
                target_entity = Some(ent.name.clone());
            }
        } else {
            let (x, y) = (
                (self.game.camera.x + 400.0).clamp(60.0, 1100.0),
                (self.game.camera.y + 300.0).clamp(60.0, 520.0),
            );
            let new_idx = self.game.spawn_entity(default_kind, x, y);
            if let Some(ent) = self.game.entities.get_mut(new_idx) {
                ent.script = Some(crate::entity::KVLScriptComponent::new(&script_path).with_overlap(trigger_event));
                attached = true;
                target_entity = Some(ent.name.clone());
            }
        }

        let diag_json: Vec<serde_json::Value> = diagnostics
            .iter()
            .map(|d| {
                serde_json::json!({
                    "severity": match d.severity { kvl_core::ValidationSeverity::Error => "error", kvl_core::ValidationSeverity::Warning => "warning" },
                    "message": d.message,
                    "node_id": d.node_id,
                    "pin_id": d.pin_id,
                })
            })
            .collect();

        serde_json::json!({
            "ok": true,
            "script_path": script_path,
            "name": graph.name,
            "description": graph.description,
            "summary": summary,
            "trigger_event": trigger_event,
            "attached": attached,
            "target_entity": target_entity,
            "node_count": graph.nodes.len(),
            "diagnostics": diag_json,
            "graph_json": graph_json,
        })
        .to_string()
    }

    /// Liste les sprites pixel-art disponibles (`assets/generated/sprites/*.png`)
    /// sous forme JSON `[{ id, file, w, h }]`, triés par identifiant.
    pub fn list_sprite_assets_json(&self) -> String {
        let roots = [
            "assets/generated/sprites".to_string(),
            format!("{}/../../assets/generated/sprites", env!("CARGO_MANIFEST_DIR")),
        ];
        let mut seen = std::collections::BTreeMap::<String, serde_json::Value>::new();
        for dir in roots {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("png") {
                        continue;
                    }
                    let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
                        continue;
                    };
                    let (w, h) = std::fs::read(&path)
                        .ok()
                        .and_then(|b| crate::vector::png_dimensions(&b))
                        .unwrap_or((0, 0));
                    seen.entry(id.to_string()).or_insert_with(|| {
                        serde_json::json!({ "id": id, "file": path.display().to_string(), "w": w, "h": h })
                    });
                }
            }
        }
        serde_json::Value::Array(seen.into_values().collect()).to_string()
    }

    /// Octets PNG bruts d'un sprite identifié (miniature dans le Studio).
    pub fn sprite_png_bytes(&self, id: &str) -> Option<Vec<u8>> {
        crate::vector::read_png_source(id)
    }

    /// Liste les animations pxl disponibles (`assets/generated/anims/*.json`)
    /// sous forme JSON `[{ id, file, frames }]`, triées par identifiant.
    pub fn list_anim_assets_json(&self) -> String {
        let roots = [
            "assets/generated/anims".to_string(),
            format!("{}/../../assets/generated/anims", env!("CARGO_MANIFEST_DIR")),
        ];
        let mut seen = std::collections::BTreeMap::<String, serde_json::Value>::new();
        for dir in roots {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("json") {
                        continue;
                    }
                    let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
                        continue;
                    };
                    let frames = std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                        .and_then(|v| v.get("frames").and_then(|f| f.as_object()).map(|o| o.len()))
                        .unwrap_or(0);
                    seen.entry(id.to_string()).or_insert_with(|| {
                        serde_json::json!({ "id": id, "file": path.display().to_string(), "frames": frames })
                    });
                }
            }
        }
        serde_json::Value::Array(seen.into_values().collect()).to_string()
    }

    /// Liste les atlas pxl (`assets/generated/<sheet>.json` à la racine) sous
    /// forme JSON `[{ id, file, frames: [noms] }]`, triés par identifiant.
    pub fn list_sheet_assets_json(&self) -> String {
        let roots = [
            "assets/generated".to_string(),
            format!("{}/../../assets/generated", env!("CARGO_MANIFEST_DIR")),
        ];
        let mut seen = std::collections::BTreeMap::<String, serde_json::Value>::new();
        for dir in roots {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("json") {
                        continue;
                    }
                    let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
                        continue;
                    };
                    // Un atlas doit avoir un PNG homonyme (les anims vivent dans anims/).
                    if !path.with_extension("png").exists() {
                        continue;
                    }
                    let frames = std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                        .and_then(|v| {
                            v.get("frames")
                                .and_then(|f| f.as_object())
                                .map(|o| o.keys().cloned().collect::<Vec<String>>())
                        })
                        .unwrap_or_default();
                    seen.entry(id.to_string()).or_insert_with(|| {
                        serde_json::json!({ "id": id, "file": path.display().to_string(), "frames": frames })
                    });
                }
            }
        }
        serde_json::Value::Array(seen.into_values().collect()).to_string()
    }

    /// Octets PNG de l'atlas (feuille) ou de la bande d'animation d'un identifiant.
    pub fn sheet_png_bytes(&self, id: &str) -> Option<Vec<u8>> {
        crate::sheet::read_sheet_source(id).map(|(png, _json)| png)
    }

    // --- Import d'images & vectorisation --------------------------------------

    fn repo_root(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn valid_id(&self, id: &str) -> Result<(), String> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err("Identifiant d'asset invalide (lettres, chiffres, _ et - uniquement).".into());
        }
        Ok(())
    }

    /// Importe un PNG (base64) comme sprite pixel-art : écrit
    /// `assets/generated/sprites/<id>.png` et le rend disponible au spawn.
    pub fn import_raster_png(&mut self, id: &str, png_base64: &str) -> Result<(u32, u32), String> {
        use base64::Engine as _;
        self.valid_id(id)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(png_base64.trim())
            .map_err(|e| format!("base64 invalide : {e}"))?;
        let img = crate::vector::decode_png(&bytes).map_err(|e| e.to_string())?;
        if img.width > 2048 || img.height > 2048 {
            return Err("Image trop grande (max 2048×2048)".into());
        }
        let path = self.repo_root().join("assets/generated/sprites").join(format!("{id}.png"));
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
        let _ = self.game.sprite_cache.get_or_load(id);
        Ok((img.width, img.height))
    }

    /// Importe un SVG (texte) : écrit `assets/sprites/<id>.svg` et valide la rastérisation.
    pub fn import_svg(&mut self, id: &str, svg_text: &str) -> Result<(u32, u32), String> {
        self.valid_id(id)?;
        let img = crate::vector::rasterize_svg(svg_text, None).map_err(|e| e.to_string())?;
        let path = self.repo_root().join("assets/sprites").join(format!("{id}.svg"));
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, svg_text).map_err(|e| e.to_string())?;
        Ok((img.width, img.height))
    }
    /// Vectorise un PNG (base64) et écrit le SVG (`assets/sprites/<id>.svg`) ainsi
    /// que le modèle éditable (`assets/vectors/<id>.vec.json`). Retourne le modèle JSON.
    pub fn vectorize_raster_asset(&self, id: &str, png_base64: &str, opts_json: &str) -> Result<String, String> {
        use base64::Engine as _;
        self.valid_id(id)?;
        let opts: crate::trace::TraceOptions = serde_json::from_str(opts_json).map_err(|e| e.to_string())?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(png_base64.trim())
            .map_err(|e| format!("base64 invalide : {e}"))?;
        let img = crate::vector::decode_png(&bytes).map_err(|e| e.to_string())?;
        let model = crate::trace::vectorize_raster(&img, &opts)?;
        let svg = model.to_svg();
        let svg_path = self.repo_root().join("assets/sprites").join(format!("{id}.svg"));
        let model_path = self.repo_root().join("assets/vectors").join(format!("{id}.vec.json"));
        if let Some(p) = svg_path.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        if let Some(p) = model_path.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        std::fs::write(&svg_path, &svg).map_err(|e| e.to_string())?;
        std::fs::write(&model_path, serde_json::to_string_pretty(&model).unwrap()).map_err(|e| e.to_string())?;
        serde_json::to_string(&model).map_err(|e| e.to_string())
    }

    /// Liste les vecteurs édités (`assets/vectors/*.vec.json`) sous forme JSON `[{ id, file }]`.
    pub fn list_vector_assets_json(&self) -> String {
        let dir = self.repo_root().join("assets/vectors");
        let mut seen = std::collections::BTreeMap::<String, serde_json::Value>::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Some(id) = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.strip_suffix(".vec")) else {
                    continue;
                };
                seen.entry(id.to_string()).or_insert_with(|| {
                    serde_json::json!({ "id": id, "file": path.display().to_string() })
                });
            }
        }
        serde_json::Value::Array(seen.into_values().collect()).to_string()
    }

    /// Charge le modèle éditable d'un vecteur (`assets/vectors/<id>.vec.json`).
    pub fn vector_load_json(&self, id: &str) -> Result<String, String> {
        self.valid_id(id)?;
        let path = self.repo_root().join("assets/vectors").join(format!("{id}.vec.json"));
        std::fs::read_to_string(&path).map_err(|e| format!("Vecteur introuvable : {e}"))
    }

    /// Enregistre un modèle éditable : réécrit `.vec.json` + le SVG correspondant.
    pub fn vector_save(&self, id: &str, model_json: &str) -> Result<String, String> {
        self.valid_id(id)?;
        let model: crate::trace::VectorModel = serde_json::from_str(model_json).map_err(|e| e.to_string())?;
        let svg = model.to_svg();
        let svg_path = self.repo_root().join("assets/sprites").join(format!("{id}.svg"));
        let model_path = self.repo_root().join("assets/vectors").join(format!("{id}.vec.json"));
        std::fs::write(&svg_path, &svg).map_err(|e| e.to_string())?;
        std::fs::write(&model_path, model_json).map_err(|e| e.to_string())?;
        Ok(svg)
    }

    /// Instancie une entité `SvgRef` pour un vecteur identifié (rastérisé à l'import).
    pub fn spawn_vector_asset(&mut self, id: &str) -> Result<usize, String> {
        let mut svg_cache = crate::vector::SvgCache::new();
        let loaded = svg_cache.get_or_load(id).map_err(|e| e.to_string())?;
        let (w, h) = (loaded.width as f32, loaded.height as f32);
        let next_id = self.game.entities.iter().map(|e| e.id).max().unwrap_or(0) + 1;
        let mut e = Entity::new(
            next_id,
            format!("Vecteur_{id}"),
            "sprite",
            Transform2D::new(0.0, 0.0, w, h),
        );
        e.renderable = Some(crate::entity::Renderable2D::svg_ref(id, 5));
        e.vector_sprite = Some(crate::entity::VectorSprite {
            asset: id.to_string(),
            image: loaded.image.clone(),
            rig: loaded.rig.clone(),
            clip: loaded
                .rig
                .as_ref()
                .and_then(|r| r.clips.first())
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "idle".to_string()),
            time: 0.0,
            speed: 1.0,
            playing: true,
        });
        self.game.entities.push(e);
        Ok(next_id)
    }


    /// Instancie une entité selon un préfabriqué (coin, potion, key, chest, door,
    /// platform, wall, spikes, boss, crate, plate).
    pub fn spawn_entity_preset(&mut self, preset: &str, x: f32, y: f32) -> usize {
        let next_id = self.game.entities.iter().map(|e| e.id).max().unwrap_or(0) + 1;
        let ent = if let Some(asset) = preset.strip_prefix("sprite:") {
            // Sprite pixel-art personnalisé (draggé depuis « Mes sprites ») :
            // dimensionné à la taille native du PNG, collider générique par défaut.
            let (w, h) = self
                .game
                .sprite_cache
                .get_or_load(asset)
                .map(|img| (img.width as f32, img.height as f32))
                .unwrap_or((32.0, 32.0));
            let mut e = crate::entity::Entity::new(
                next_id,
                format!("Sprite_{asset}"),
                "sprite",
                crate::entity::Transform2D::new(x, y, w, h),
            );
            e.renderable = Some(crate::entity::Renderable2D::image_ref(asset, 5));
            e
        } else if let Some(asset) = preset.strip_prefix("anim:") {
            // Animation pxl (bande de frames) dimensionnée à la frame 0.
            let (w, h) = self
                .game
                .sheet_cache
                .get_or_load(asset)
                .ok()
                .and_then(|s| s.frame_index(0).map(|f| (f.image.width as f32, f.image.height as f32)))
                .unwrap_or((32.0, 32.0));
            let mut e = crate::entity::Entity::new(
                next_id,
                format!("Anim_{asset}"),
                "anim",
                crate::entity::Transform2D::new(x, y, w, h),
            );
            e.renderable = Some(crate::entity::Renderable2D::anim_ref(asset, 5));
            e
        } else if let Some(rest) = preset.strip_prefix("sheet:") {
            // Frame nommée d'une spritesheet pxl : `sheet:<asset>:<frame>`.
            let (asset, frame) = rest.split_once(':').unwrap_or((rest, "coin"));
            let (w, h) = self
                .game
                .sheet_cache
                .get_or_load(asset)
                .ok()
                .and_then(|s| s.frame(frame).map(|f| (f.image.width as f32, f.image.height as f32)))
                .unwrap_or((32.0, 32.0));
            let mut e = crate::entity::Entity::new(
                next_id,
                format!("Sheet_{frame}"),
                "sprite",
                crate::entity::Transform2D::new(x, y, w, h),
            );
            e.renderable = Some(crate::entity::Renderable2D::sheet_ref(asset, frame, 5));
            e
        } else {
            match preset {
            "coin" | "collectible" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Piece_{:02}", next_id),
                    "collectible",
                    crate::entity::Transform2D::new(x, y, 24.0, 24.0),
                );
                e.collider = Some(crate::entity::Collider2D::trigger(24.0, 24.0));
                e.renderable = Some(crate::entity::Renderable2D::image_ref("coin", 5));
                e
            }
            "potion" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Potion_{:02}", next_id),
                    "item",
                    crate::entity::Transform2D::new(x, y, 24.0, 24.0),
                );
                e.collider = Some(crate::entity::Collider2D::trigger(24.0, 24.0));
                e.renderable = Some(crate::entity::Renderable2D::image_ref("potion", 5));
                e
            }
            "key" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Cle_{:02}", next_id),
                    "item",
                    crate::entity::Transform2D::new(x, y, 24.0, 24.0),
                );
                e.collider = Some(crate::entity::Collider2D::trigger(24.0, 24.0));
                e.renderable = Some(crate::entity::Renderable2D::image_ref("key", 2));
                e
            }
            "chest" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Coffre_{:02}", next_id),
                    "item",
                    crate::entity::Transform2D::new(x, y, 36.0, 36.0),
                );
                e.collider = Some(crate::entity::Collider2D::solid(36.0, 36.0));
                e.renderable = Some(crate::entity::Renderable2D::image_ref("chest", 3));
                e
            }
            "door" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Porte_{:02}", next_id),
                    "door",
                    crate::entity::Transform2D::new(x, y, 32.0, 64.0),
                );
                e.collider = Some(crate::entity::Collider2D::solid(32.0, 64.0));
                e.renderable = Some(crate::entity::Renderable2D::image_ref("door", 2));
                e
            }
            "platform" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Plateforme_{:02}", next_id),
                    "platform",
                    crate::entity::Transform2D::new(x, y, 160.0, 24.0),
                );
                e.collider = Some(crate::entity::Collider2D::solid(160.0, 24.0));
                e.renderable = Some(crate::entity::Renderable2D::image_ref("platform", 1));
                e
            }
            "spikes" | "hazard" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Piques_{:02}", next_id),
                    "hazard",
                    crate::entity::Transform2D::new(x, y, 96.0, 20.0),
                );
                e.collider = Some(crate::entity::Collider2D::trigger(96.0, 20.0));
                e.renderable = Some(crate::entity::Renderable2D::image_ref("spikes", 3));
                e
            }
            "crate" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Caisse_{:02}", next_id),
                    "crate",
                    crate::entity::Transform2D::new(x, y, 32.0, 32.0),
                );
                let mut c = crate::entity::Collider2D::solid(32.0, 32.0);
                c.is_pushable = true;
                e.collider = Some(c);
                e.renderable = Some(crate::entity::Renderable2D::image_ref("crate", 5));
                e
            }
            "plate" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Dalle_{:02}", next_id),
                    "plate",
                    crate::entity::Transform2D::new(x, y, 32.0, 32.0),
                );
                e.collider = Some(crate::entity::Collider2D::trigger(32.0, 32.0));
                e.renderable = Some(crate::entity::Renderable2D::image_ref("plate", 2));
                e
            }
            "boss" => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Boss_{:02}", next_id),
                    "boss",
                    crate::entity::Transform2D::new(x, y, 72.0, 72.0),
                );
                e.collider = Some(crate::entity::Collider2D::solid(72.0, 72.0));
                e.health = Some(crate::entity::Health2D::new(100.0));
                e.renderable = Some(crate::entity::Renderable2D::image_ref("boss", 4));
                e
            }
            _ => {
                let mut e = crate::entity::Entity::new(
                    next_id,
                    format!("Mur_{:02}", next_id),
                    "wall",
                    crate::entity::Transform2D::new(x, y, 32.0, 32.0),
                );
                e.collider = Some(crate::entity::Collider2D::solid(32.0, 32.0));
                e.renderable = Some(crate::entity::Renderable2D::image_ref("wall", 1));
                e
            }
            }
        };
        self.game.entities.push(ent);
        let new_idx = self.game.entities.len() - 1;
        // Sélection immédiate + commande d'annulation (Ctrl+Z) pour la palette.
        self.game.editor.selected = crate::editor::SelectedEntity::Entity(new_idx);
        self.game.editor.undo_stack.push_undo(crate::editor::StudioCommand::SpawnEntity {
            entity_id: next_id,
            entity_name: self.game.entities[new_idx].name.clone(),
            cell: None,
        });
        self.game
            .set_status(format!("Entité #{} ('{}') générée avec succès.", next_id, preset));
        next_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guided_graph_builds_valid_mechanics() {
        let mut engine = NativeEngine::init().expect("init moteur");

        let recipes = ["collectible", "hazard", "teleport", "switch", "chest", "door"];
        for recipe in recipes {
            let params = serde_json::json!({
                "entity_id": null,
                "item_name": "Piece Test",
                "hazard_name": "Lave",
                "portal_name": "Portail",
                "switch_name": "Dalle",
                "chest_name": "Coffre",
                "door_name": "Porte",
                "score_points": 10,
                "damage_amount": 15,
                "reward_score": 100,
                "trigger": "proximity",
                "condition": "none",
            });
            let out = engine.build_guided_graph(recipe, params);
            let v: serde_json::Value = serde_json::from_str(&out).expect("enveloppe JSON valide");
            assert_eq!(v["ok"], true, "recette {} => {}", recipe, out);
            assert!(v["graph_json"].is_string());

            // Le graphe généré doit être valide (aucune erreur bloquante).
            let diags = v["diagnostics"].as_array().unwrap();
            assert!(
                !diags.iter().any(|d| d["severity"] == "error"),
                "recette {} produit une erreur : {}",
                recipe,
                out
            );
        }
    }

    #[test]
    fn test_guided_graph_rejects_unknown_recipe() {
        let mut engine = NativeEngine::init().expect("init moteur");
        let out = engine.build_guided_graph("n_importe_quoi", serde_json::json!({}));
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], false);
    }

    #[test]
    fn test_spawn_presets_are_visible_with_renderable() {
        let mut engine = NativeEngine::init().expect("init moteur");
        let before = engine.game.entities.len();

        for preset in ["coin", "potion", "key", "chest", "door", "platform", "spikes", "crate", "plate", "boss", "wall"] {
            let id = engine.spawn_entity_preset(preset, 100.0, 100.0);
            let ent = engine.game.entities.iter().find(|e| e.id == id).expect("entité créée");
            assert!(
                ent.renderable.is_some(),
                "le préfabriqué '{}' doit avoir un rendu visible",
                preset
            );
        }

        assert_eq!(engine.game.entities.len(), before + 11);
    }

    #[test]
    fn test_spawn_sprite_asset_uses_native_png_size() {
        let mut engine = NativeEngine::init().expect("init moteur");
        let id = engine.spawn_entity_preset("sprite:coin", 0.0, 0.0);
        let ent = engine.game.entities.iter().find(|e| e.id == id).unwrap();
        assert!(matches!(
            &ent.renderable.as_ref().unwrap().shape,
            crate::entity::ShapeKind::ImageRef { asset } if asset == "coin"
        ));
        // Le sprite doit être chargé dans le cache et l'entité dimensionnée
        // à la taille native du PNG (coin : 18x16).
        let img = engine.game.sprite_cache.get("coin").expect("coin décodé");
        assert_eq!((img.width, img.height), (18, 16));
        assert_eq!(ent.transform.w, 18.0);
        assert_eq!(ent.transform.h, 16.0);
    }

    #[test]
    fn test_render_frame_with_image_ref_does_not_panic() {
        let mut engine = NativeEngine::init().expect("init moteur");
        engine.spawn_entity_preset("sprite:coin", 100.0, 100.0);
        engine.render_frame(1.0 / 60.0);
        // Un asset inexistant retombe sur un rectangle coloré sans paniquer.
        engine.spawn_entity_preset("sprite:introuvable", 200.0, 200.0);
        engine.render_frame(1.0 / 60.0);
    }

    #[test]
    fn test_spawn_sheet_and_anim_assets() {
        let mut engine = NativeEngine::init().expect("init moteur");

        // Frame nommée d'une spritesheet.
        let id = engine.spawn_entity_preset("sheet:chars:coin", 0.0, 0.0);
        let ent = engine.game.entities.iter().find(|e| e.id == id).unwrap();
        assert!(matches!(
            &ent.renderable.as_ref().unwrap().shape,
            crate::entity::ShapeKind::SheetRef { asset, frame } if asset == "chars" && frame == "coin"
        ));

        // Animation par frames : la frame avance dans le temps (fps 10 -> 100 ms/frame).
        let anim_id = engine.spawn_entity_preset("anim:coin_spin", 100.0, 100.0);
        let before = engine.game.anim_states.get(&anim_id).map(|s| s.frame).unwrap_or(0);
        for _ in 0..120 {
            engine.game.update(SIM_DT, &engine.input);
        }
        let after = engine.game.anim_states.get(&anim_id).map(|s| s.frame).unwrap_or(0);
        assert_ne!(before, after, "l'animation doit avancer dans le temps");

        // Le rendu des deux formes ne panique pas.
        engine.render_frame(1.0 / 60.0);
    }

    #[test]
    fn test_export_package_embeds_sprite_assets_and_import_resolves() {
        let mut engine = NativeEngine::init().expect("init moteur");
        engine.spawn_entity_preset("sprite:coin", 0.0, 0.0);
        engine.spawn_entity_preset("sheet:chars:chest", 40.0, 40.0);

        // Export : les assets doivent être embarqués (base64).
        let package_json = engine.export_game_package("Test", "Moi");
        let package: crate::exporter::KaractiGamePackage =
            serde_json::from_str(&package_json).expect("package valide");
        assert!(package.sprite_assets.contains_key("coin"), "coin embarqué");
        assert!(package.sheet_assets.contains_key("chars"), "chars embarqué");
        assert!(!package.sprite_assets["coin"].is_empty());

        // Import dans un moteur "neuf" : les assets embarqués doivent être résolus.
        let mut fresh = NativeEngine::init().expect("init moteur");
        let before = fresh.game.entities.len();
        let code = fresh.import_game_package(&package_json);
        assert_eq!(code, 0);
        assert!(fresh.game.entities.len() > before, "entités importées");
        assert!(fresh.game.sprite_cache.get("coin").is_some(), "coin résolu depuis l'embarqué");
        assert!(fresh.game.sheet_cache.get("chars").is_some(), "chars résolu depuis l'embarqué");
    }

    #[test]
    fn test_swizzle_xrgb_to_rgba() {
        // 0x00RRGGBB -> 0xAABBGGRR (ordre mémoire R, G, B, A)
        assert_eq!(swizzle_xrgb_to_rgba(0x00FF0000), 0xFF0000FF); // rouge
        assert_eq!(swizzle_xrgb_to_rgba(0x0000FF00), 0xFF00FF00); // vert
        assert_eq!(swizzle_xrgb_to_rgba(0x000000FF), 0xFFFF0000); // bleu
        assert_eq!(swizzle_xrgb_to_rgba(0x00000000), 0xFF000000); // noir opaque
    }

    #[test]
    fn test_frame_bytes_encodes_rgba_order() {
        let mut engine = NativeEngine::init().expect("init moteur");
        // Pixel unique : noir opaque, donc RGBA = [0, 0, 0, 255]
        let n = engine.canvas.buffer.len();
        engine.canvas.buffer[0] = 0x00000000; // XRGB noir
        let bytes = engine.frame_bytes();
        assert_eq!(bytes.len(), n * 4);
        assert_eq!(&bytes[0..4], &[0, 0, 0, 255]);

        // Rouge pur XRGB 0x00FF0000 -> RGBA [255, 0, 0, 255]
        engine.canvas.buffer[0] = 0x00FF0000;
        let bytes = engine.frame_bytes();
        assert_eq!(&bytes[0..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn test_fixed_timestep_clamps_substeps() {
        let mut engine = NativeEngine::init().expect("init moteur");

        // Scénario 1 : un énorme dt (0.5 s) est clampé à 0.25 s dans l'accumulateur,
        // soit 30 pas de 120 Hz, mais plafonné à MAX_SUBSTEPS (4).
        engine.accumulator = 0.0;
        let before = engine.game.sim_tick;
        engine.render_frame(0.5);
        let steps = engine.game.sim_tick - before;
        assert_eq!(steps, 4u64, "un grand dt doit être plafonné à 4 sous-pas");

        // Scénario 2 : un dt nominal (1/60 s) doit produire exactement 2 sous-pas.
        engine.accumulator = 0.0;
        let before = engine.game.sim_tick;
        engine.render_frame(1.0 / 60.0);
        let steps = engine.game.sim_tick - before;
        assert_eq!(steps, 2u64, "1/60 s = 2 sous-pas de 1/120 s");
    }

    #[test]
    fn test_launch_default_game_switches_scene_and_stays_embedded() {
        let mut engine = NativeEngine::init().expect("init moteur");

        // L'alias historique 'boss_arena' (auparavant cassé) doit résoudre boss_01.
        let name = engine.launch_default_game("boss_arena").expect("lance le jeu par défaut boss");
        assert!(
            name.contains("Titan") || name.contains("Boss") || name.contains("Sentinelle"),
            "nom réel de la scène : {}",
            name
        );
        assert!(engine.game.scene_path.contains("boss_01"), "scène boss chargée : {}", engine.game.scene_path);
        assert!(engine.game.editor.embedded_mode, "le mode embarqué doit être préservé");
        assert!(matches!(engine.game.editor.mode, crate::editor::EditorMode::Play));
        assert!(!engine.game.editor.show_welcome_modal);
    }

    #[test]
    fn test_move_entity_3d_updates_position() {
        let mut engine = NativeEngine::init().expect("init moteur");
        engine.populate_default_3d_scene();
        let cube_id = engine
            .game
            .entities
            .iter()
            .find(|e| {
                e.transform3d.is_some()
                    && matches!(&e.renderable3d.as_ref().map(|r| &r.mesh), Some(MeshRef::Cube))
            })
            .map(|e| e.id)
            .expect("au moins un cube dans la scène par défaut");

        assert!(engine.move_entity_3d(cube_id, 3.0, 4.0));
        let ent = engine.game.entities.iter().find(|e| e.id == cube_id).unwrap();
        let t3d = ent.transform3d.as_ref().unwrap();
        assert_eq!(t3d.position.x, 3.0);
        assert_eq!(t3d.position.z, 4.0);
        // Une entité inexistante ne doit rien faire.
        assert!(!engine.move_entity_3d(usize::MAX, 0.0, 0.0));
    }
}
