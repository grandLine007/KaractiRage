use crate::game::GameState;
use crate::input::InputState;
use crate::renderer::Canvas;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use winit::keyboard::KeyCode;

pub const CANVAS_WIDTH: usize = 1200;
pub const CANVAS_HEIGHT: usize = 800;

struct WasmRuntime {
    game: GameState,
    canvas: Canvas,
    input: InputState,
    rgba_buffer: Vec<u32>,
}

thread_local! {
    static RUNTIME: RefCell<Option<WasmRuntime>> = const { RefCell::new(None) };
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
        _ => None,
    }
}

/// Initialise l'instance WebAssembly du moteur KaractiRage
#[wasm_bindgen]
pub fn wasm_init_engine() -> i32 {
    let mut game = match GameState::load_from_scene("assets/scenes/level_01.scene.json") {
        Ok(g) => g,
        Err(_) => return -1,
    };
    game.editor.embedded_mode = true;
    game.editor.show_welcome_modal = false;
    game.editor.show_help_modal = false;

    let canvas = Canvas::new(CANVAS_WIDTH, CANVAS_HEIGHT);
    crate::renderer::warmup_glyph_cache();
    let input = InputState::new();
    let rgba_buffer = vec![0u32; CANVAS_WIDTH * CANVAS_HEIGHT];

    RUNTIME.with(|rt| {
        *rt.borrow_mut() = Some(WasmRuntime {
            game,
            canvas,
            input,
            rgba_buffer,
        });
    });

    0
}

/// Retourne la largeur native du Canvas (1200 px)
#[wasm_bindgen]
pub fn wasm_get_canvas_width() -> u32 {
    CANVAS_WIDTH as u32
}

/// Retourne la hauteur native du Canvas (800 px)
#[wasm_bindgen]
pub fn wasm_get_canvas_height() -> u32 {
    CANVAS_HEIGHT as u32
}

/// Retourne le pointeur brut vers le buffer de pixels RGBA en mémoire linéaire WebAssembly
#[wasm_bindgen]
pub fn wasm_get_framebuffer_ptr() -> *const u32 {
    RUNTIME.with(|rt| {
        if let Ok(borrow) = rt.try_borrow() {
            if let Some(ref r) = *borrow {
                r.rgba_buffer.as_ptr()
            } else {
                std::ptr::null()
            }
        } else {
            std::ptr::null()
        }
    })
}

/// Enregistre un appui touche depuis le navigateur
#[wasm_bindgen]
pub fn wasm_on_key_down(key: u32) {
    if let Some(kc) = web_key_to_keycode(key) {
        RUNTIME.with(|rt| {
            if let Ok(mut borrow) = rt.try_borrow_mut() {
                if let Some(ref mut r) = *borrow {
                    r.input.on_key_down(kc);
                }
            }
        });
    }
}

/// Enregistre un relâchement touche depuis le navigateur
#[wasm_bindgen]
pub fn wasm_on_key_up(key: u32) {
    if let Some(kc) = web_key_to_keycode(key) {
        RUNTIME.with(|rt| {
            if let Ok(mut borrow) = rt.try_borrow_mut() {
                if let Some(ref mut r) = *borrow {
                    r.input.on_key_up(kc);
                }
            }
        });
    }
}

/// Enregistre le déplacement de la souris sur le Canvas
#[wasm_bindgen]
pub fn wasm_on_mouse_move(x: f32, y: f32) {
    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                r.input.on_mouse_move(x, y);
            }
        }
    });
}

/// Enregistre un clic de souris (bouton 0: gauche, bouton 2: droit)
#[wasm_bindgen]
pub fn wasm_on_mouse_down(button: u32) {
    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                if button == 0 {
                    r.input.on_mouse_down();
                } else if button == 2 {
                    r.input.on_mouse_right_click();
                }
            }
        }
    });
}

/// Enregistre le relâchement du clic de souris
#[wasm_bindgen]
pub fn wasm_on_mouse_up(button: u32) {
    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                if button == 0 {
                    r.input.on_mouse_up();
                }
            }
        }
    });
}

/// Conversion vectorisée/déroulée ultra-rapide du format softbuffer 0x00RRGGBB vers RGBA32 pour Canvas HTML5
#[inline]
pub fn convert_xrgb_to_rgba(src: &[u32], dst: &mut [u32]) {
    let len = src.len().min(dst.len());
    let chunks = len / 4;
    let remainder = len % 4;

    // Déroulage 4x permettant au compilateur LLVM d'émettre des instructions SIMD (v128 / NEON / AVX2)
    for c in 0..chunks {
        let base = c * 4;
        let col0 = src[base];
        let col1 = src[base + 1];
        let col2 = src[base + 2];
        let col3 = src[base + 3];

        dst[base] = 0xFF000000 | ((col0 & 0x000000FF) << 16) | (col0 & 0x0000FF00) | ((col0 & 0x00FF0000) >> 16);
        dst[base + 1] = 0xFF000000 | ((col1 & 0x000000FF) << 16) | (col1 & 0x0000FF00) | ((col1 & 0x00FF0000) >> 16);
        dst[base + 2] = 0xFF000000 | ((col2 & 0x000000FF) << 16) | (col2 & 0x0000FF00) | ((col2 & 0x00FF0000) >> 16);
        dst[base + 3] = 0xFF000000 | ((col3 & 0x000000FF) << 16) | (col3 & 0x0000FF00) | ((col3 & 0x00FF0000) >> 16);
    }

    let tail = chunks * 4;
    for i in 0..remainder {
        let col = src[tail + i];
        dst[tail + i] = 0xFF000000 | ((col & 0x000000FF) << 16) | (col & 0x0000FF00) | ((col & 0x00FF0000) >> 16);
    }
}

/// Avance la simulation physique d'un pas dt et génère le rendu dans le buffer RGBA
#[wasm_bindgen]
pub fn wasm_render_frame(dt: f32) -> u32 {
    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                // 1. Mise à jour simulation & KVL
                r.game.update(dt, &r.input);

                // 2. Rendu logiciel dans le Canvas
                r.game.render(&mut r.canvas);

                // 3. Conversion optimisée softbuffer 0x00RRGGBB vers RGBA32 pour Canvas HTML5
                convert_xrgb_to_rgba(&r.canvas.buffer, &mut r.rgba_buffer);

                // 4. Nettoyage des événements transitoires d'entrée
                r.input.clear_transient();

                1
            } else {
                0
            }
        } else {
            0
        }
    })
}

/// Récupère le score KVL actuel
#[wasm_bindgen]
pub fn wasm_get_score() -> i32 {
    RUNTIME.with(|rt| {
        if let Ok(borrow) = rt.try_borrow() {
            if let Some(ref r) = *borrow {
                r.game.kvl_context.get_var("score").map_or(0, |v| match v { kvl_core::Value::Int(s) => *s as i32, _ => 0 })
            } else {
                0
            }
        } else {
            0
        }
    })
}

/// Récupère le FPS mesuré par le module de télémétrie
#[wasm_bindgen]
pub fn wasm_get_fps() -> f32 {
    RUNTIME.with(|rt| {
        if let Ok(borrow) = rt.try_borrow() {
            if let Some(ref r) = *borrow {
                r.game.telemetry.current_fps
            } else {
                0.0
            }
        } else {
            0.0
        }
    })
}

/// Récupère le temps moyen de calcul CPU par frame en microsecondes
#[wasm_bindgen]
pub fn wasm_get_cpu_us() -> u32 {
    RUNTIME.with(|rt| {
        if let Ok(borrow) = rt.try_borrow() {
            if let Some(ref r) = *borrow {
                r.game.telemetry.avg_frame_time_us as u32
            } else {
                0
            }
        } else {
            0
        }
    })
}

/// Récupère le nombre de requêtes spatiales de la dernière frame
#[wasm_bindgen]
pub fn wasm_get_spatial_queries() -> u32 {
    RUNTIME.with(|rt| {
        if let Ok(borrow) = rt.try_borrow() {
            if let Some(ref r) = *borrow {
                r.game.spatial_grid.stats_queries as u32
            } else {
                0
            }
        } else {
            0
        }
    })
}

/// Bascule la minicarte radar
#[wasm_bindgen]
pub fn wasm_toggle_radar() -> u32 {
    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                r.game.show_minimap = !r.game.show_minimap;
                if r.game.show_minimap { 1 } else { 0 }
            } else {
                0
            }
        } else {
            0
        }
    })
}

/// Bascule la grille de partitionnement spatial
#[wasm_bindgen]
pub fn wasm_toggle_spatial_grid() -> u32 {
    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                let vis = r.game.spatial_grid.toggle_debug();
                if vis { 1 } else { 0 }
            } else {
                0
            }
        } else {
            0
        }
    })
}

/// Bascule le mode éditeur KVL
#[wasm_bindgen]
pub fn wasm_toggle_editor_mode() -> u32 {
    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                r.game.editor.toggle_mode();
                match r.game.editor.mode {
                    crate::editor::EditorMode::Edit => 1,
                    crate::editor::EditorMode::Play => 0,
                }
            } else {
                0
            }
        } else {
            0
        }
    })
}

/// Active ou désactive le mode embarqué (désactive les barres chromées softbuffer pour laisser React gérer le HUD/Studio)
#[wasm_bindgen]
pub fn wasm_set_embedded_mode(enabled: bool) {
    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                r.game.editor.embedded_mode = enabled;
            }
        }
    });
}

/// Récupère le message de statut courant (UTF-8)
#[wasm_bindgen]
pub fn wasm_get_status_message() -> String {
    RUNTIME.with(|rt| {
        if let Ok(borrow) = rt.try_borrow() {
            if let Some(ref r) = *borrow {
                r.game.status_message.clone()
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    })
}

/// Exporte le jeu courant en fichier HTML5 standalone complet autonome
#[wasm_bindgen]
pub fn wasm_export_standalone_html(title: &str, author: &str) -> String {
    RUNTIME.with(|rt| {
        if let Ok(borrow) = rt.try_borrow() {
            if let Some(ref r) = *borrow {
                crate::exporter::StandaloneExporter::export_html(&r.game, title, author)
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    })
}

/// Exporte le package universel KaractiRage (.karacti.json)
#[wasm_bindgen]
pub fn wasm_export_game_package(title: &str, author: &str) -> String {
    RUNTIME.with(|rt| {
        if let Ok(borrow) = rt.try_borrow() {
            if let Some(ref r) = *borrow {
                crate::exporter::StandaloneExporter::export_package_json(&r.game, title, author)
                    .unwrap_or_else(|_| "{}".to_string())
            } else {
                "{}".to_string()
            }
        } else {
            "{}".to_string()
        }
    })
}

/// Importe et instancie un package universel (.karacti.json) dans le moteur
#[wasm_bindgen]
pub fn wasm_import_game_package(package_json: &str) -> i32 {
    let package = match crate::exporter::KaractiGamePackage::from_json(package_json) {
        Ok(p) => p,
        Err(_) => return -1,
    };

    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                crate::exporter::StandaloneExporter::import_package(&mut r.game, &package);
                0
            } else {
                -2
            }
        } else {
            -2
        }
    })
}

/// Valide une recette de jeu (JSON) et résout automatiquement ses dépendances
#[wasm_bindgen]
pub fn wasm_validate_recipe(recipe_json: &str) -> String {
    let mut recipe = match crate::recipe::KaractiGameRecipe::from_json(recipe_json) {
        Ok(r) => r,
        Err(e) => return format!("{{\"is_valid\":false,\"conflicts\":[\"JSON invalide: {}\"]}}", e),
    };

    let report = crate::recipe::DependencyResolver::resolve(&mut recipe);
    serde_json::to_string(&report).unwrap_or_else(|_| "{}".to_string())
}

/// Instancie une recette de jeu et charge la scène minimale jouable dans le moteur
#[wasm_bindgen]
pub fn wasm_apply_recipe(recipe_json: &str) -> i32 {
    let mut recipe = match crate::recipe::KaractiGameRecipe::from_json(recipe_json) {
        Ok(r) => r,
        Err(_) => return -1,
    };

    let report = crate::recipe::DependencyResolver::resolve(&mut recipe);
    if !report.is_valid {
        return -2;
    }

    let (scene, kvl_graphs) = crate::recipe::RecipeProjectGenerator::generate(&recipe);

    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                r.game.scene_name = scene.name;
                r.game.entities = scene.entities;
                r.game.kvl_graphs = kvl_graphs;
                r.game.editor.selected = crate::editor::SelectedEntity::None;
                r.game.editor.mode = crate::editor::EditorMode::Play;
                r.game.set_status(&format!("Recette '{}' instanciée avec succès !", recipe.title));
                0
            } else {
                -3
            }
        } else {
            -3
        }
    })
}

/// Applique un payload IA (format KARACTI_AI_SPEC.md) directement dans la scène active
#[wasm_bindgen]
pub fn wasm_apply_ai_payload(payload_json: &str, prompt: &str) -> i32 {
    let payload: crate::ai_client::AiGeneratedGamePayload = match serde_json::from_str(payload_json) {
        Ok(p) => p,
        Err(_) => return -1,
    };

    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                r.game.apply_ai_payload(payload, prompt);
                0
            } else {
                -2
            }
        } else {
            -2
        }
    })
}

/// Valide définitivement le patch IA actif (retire les halos violets)
#[wasm_bindgen]
pub fn wasm_accept_ai_patch() -> i32 {
    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                r.game.accept_ai_patch();
                0
            } else {
                -1
            }
        } else {
            -1
        }
    })
}

/// Rejette le patch IA et applique un rollback instantané vers l'état antérieur
#[wasm_bindgen]
pub fn wasm_reject_ai_patch() -> i32 {
    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                r.game.reject_ai_patch();
                0
            } else {
                -1
            }
        } else {
            -1
        }
    })
}

/// Indique si un patch IA est en attente de révision dans le moteur (1 = oui, 0 = non)
#[wasm_bindgen]
pub fn wasm_has_pending_ai_patch() -> i32 {
    RUNTIME.with(|rt| {
        if let Ok(borrow) = rt.try_borrow() {
            if let Some(ref r) = *borrow {
                if r.game.editor.pending_ai_patch.is_some() { 1 } else { 0 }
            } else {
                0
            }
        } else {
            0
        }
    })
}

/// Commute l'outil actif de l'atelier Studio (0 = Select/V, 1 = Brush/B, 2 = Eraser/X)
#[wasm_bindgen]
pub fn wasm_set_active_tool(tool: u32) -> u32 {
    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                let tool_enum = match tool {
                    0 => crate::editor::StudioTool::Select,
                    1 => crate::editor::StudioTool::Brush,
                    2 => crate::editor::StudioTool::Eraser,
                    _ => crate::editor::StudioTool::Select,
                };
                r.game.editor.set_tool(tool_enum);
                tool
            } else {
                0
            }
        } else {
            0
        }
    })
}

/// Récupère l'outil actif de l'atelier (0 = Select, 1 = Brush, 2 = Eraser)
#[wasm_bindgen]
pub fn wasm_get_active_tool() -> u32 {
    RUNTIME.with(|rt| {
        if let Ok(borrow) = rt.try_borrow() {
            if let Some(ref r) = *borrow {
                match r.game.editor.active_tool {
                    crate::editor::StudioTool::Select => 0,
                    crate::editor::StudioTool::Brush => 1,
                    crate::editor::StudioTool::Eraser => 2,
                }
            } else {
                0
            }
        } else {
            0
        }
    })
}

/// Fait défiler la tuile active du pinceau et retourne son libellé en français
#[wasm_bindgen]
pub fn wasm_cycle_brush_tile() -> String {
    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                let next = r.game.editor.cycle_brush_tile();
                next.label().to_string()
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    })
}

/// Retourne le nom de la tuile active du pinceau
#[wasm_bindgen]
pub fn wasm_get_active_brush_tile() -> String {
    RUNTIME.with(|rt| {
        if let Ok(borrow) = rt.try_borrow() {
            if let Some(ref r) = *borrow {
                r.game.editor.brush_tile.label().to_string()
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    })
}

/// Ordonne au joueur de naviguer vers une position cible avec évitement d'obstacles A*
#[wasm_bindgen]
pub fn wasm_move_player_to(target_x: f32, target_y: f32) -> bool {
    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                if let Some(p_idx) = r.game.player_index() {
                    let mut nav = crate::navigation::NavAgent::go_to(260.0, target_x, target_y);
                    nav.reach_distance = 18.0;
                    nav.avoid_hazards = true;
                    r.game.entities[p_idx].nav_agent = Some(nav);
                    r.game.status_message = format!("Navigation A* vers ({:.0}, {:.0}) enclenchée.", target_x, target_y);
                    return true;
                }
            }
        }
        false
    })
}

/// Récupère la coordonnée X actuelle du joueur
#[wasm_bindgen]
pub fn wasm_get_player_pos_x() -> f32 {
    RUNTIME.with(|rt| {
        if let Ok(borrow) = rt.try_borrow() {
            if let Some(ref r) = *borrow {
                if let Some(p_idx) = r.game.player_index() {
                    return r.game.entities[p_idx].transform.x;
                }
            }
        }
        0.0
    })
}

/// Récupère la coordonnée Y actuelle du joueur
#[wasm_bindgen]
pub fn wasm_get_player_pos_y() -> f32 {
    RUNTIME.with(|rt| {
        if let Ok(borrow) = rt.try_borrow() {
            if let Some(ref r) = *borrow {
                if let Some(p_idx) = r.game.player_index() {
                    return r.game.entities[p_idx].transform.y;
                }
            }
        }
        0.0
    })
}

/// Déclenche une interaction (Touche E)
#[wasm_bindgen]
pub fn wasm_interact() -> bool {
    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                r.input.on_key_down(winit::keyboard::KeyCode::KeyE);
                return true;
            }
        }
        false
    })
}

/// Initialise un nouveau projet vierge dans le moteur (Studio direct)
#[wasm_bindgen]
pub fn wasm_new_blank_project(title: &str, genre_str: &str) -> i32 {
    let genre = match genre_str.to_lowercase().as_str() {
        "platformer" => crate::recipe::GameGenre::Platformer,
        "shmup" | "shoot_em_up" => crate::recipe::GameGenre::ShootEmUp,
        "puzzle" | "sokoban" => crate::recipe::GameGenre::Puzzle,
        "combat" | "boss" => crate::recipe::GameGenre::Combat,
        _ => crate::recipe::GameGenre::Adventure,
    };

    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                r.game.new_blank_project(title, genre);
                0
            } else {
                -1
            }
        } else {
            -1
        }
    })
}

/// Charge un projet KaractiProject complet depuis son format JSON (.karacti)
#[wasm_bindgen]
pub fn wasm_load_project_json(project_json: &str) -> i32 {
    let proj = match crate::project::KaractiProject::from_json(project_json) {
        Ok(p) => p,
        Err(_) => return -2,
    };

    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                r.game.load_project(proj);
                0
            } else {
                -1
            }
        } else {
            -1
        }
    })
}

/// Exporte l'état complet du jeu actuel en format JSON KaractiProject (.karacti)
#[wasm_bindgen]
pub fn wasm_export_project_json() -> String {
    RUNTIME.with(|rt| {
        if let Ok(borrow) = rt.try_borrow() {
            if let Some(ref r) = *borrow {
                let proj = r.game.export_current_project();
                proj.to_json().unwrap_or_else(|_| "{}".to_string())
            } else {
                "{}".to_string()
            }
        } else {
            "{}".to_string()
        }
    })
}

/// Exporte la liste résumée de toutes les entités actives en mémoire au format JSON
#[wasm_bindgen]
pub fn wasm_get_entities_summary_json() -> String {
    RUNTIME.with(|rt| {
        if let Ok(borrow) = rt.try_borrow() {
            if let Some(ref r) = *borrow {
                r.game.get_entities_summary_json()
            } else {
                "[]".to_string()
            }
        } else {
            "[]".to_string()
        }
    })
}

/// Met à jour les coordonnées et dimensions d'une entité par son ID
#[wasm_bindgen]
pub fn wasm_update_entity_transform(id: usize, x: f32, y: f32, w: f32, h: f32) -> bool {
    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                return r.game.update_entity_transform_by_id(id, x, y, w, h);
            }
        }
        false
    })
}

/// Supprime une entité par son ID
#[wasm_bindgen]
pub fn wasm_delete_entity(id: usize) -> bool {
    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                return r.game.delete_entity_by_id(id);
            }
        }
        false
    })
}

/// Instancie une entité selon un préfabriqué (coin, potion, key, chest, door, platform, wall, spikes, boss, crate, plate)
#[wasm_bindgen]
pub fn wasm_spawn_entity_preset(preset: &str, x: f32, y: f32) -> usize {
    RUNTIME.with(|rt| {
        if let Ok(mut borrow) = rt.try_borrow_mut() {
            if let Some(ref mut r) = *borrow {
                let next_id = r.game.entities.iter().map(|e| e.id).max().unwrap_or(0) + 1;
                let ent = match preset {
                    "coin" | "collectible" => {
                        let mut e = crate::entity::Entity::new(next_id, format!("Piece_{:02}", next_id), "collectible", crate::entity::Transform2D::new(x, y, 24.0, 24.0));
                        e.collider = Some(crate::entity::Collider2D::trigger(24.0, 24.0));
                        e
                    }
                    "potion" => {
                        let mut e = crate::entity::Entity::new(next_id, format!("Potion_{:02}", next_id), "item", crate::entity::Transform2D::new(x, y, 24.0, 24.0));
                        e.collider = Some(crate::entity::Collider2D::trigger(24.0, 24.0));
                        e
                    }
                    "key" => {
                        let mut e = crate::entity::Entity::new(next_id, format!("Cle_{:02}", next_id), "item", crate::entity::Transform2D::new(x, y, 24.0, 24.0));
                        e.collider = Some(crate::entity::Collider2D::trigger(24.0, 24.0));
                        e
                    }
                    "chest" => {
                        let mut e = crate::entity::Entity::new(next_id, format!("Coffre_{:02}", next_id), "item", crate::entity::Transform2D::new(x, y, 36.0, 36.0));
                        e.collider = Some(crate::entity::Collider2D::solid(36.0, 36.0));
                        e
                    }
                    "door" => {
                        let mut e = crate::entity::Entity::new(next_id, format!("Porte_{:02}", next_id), "door", crate::entity::Transform2D::new(x, y, 32.0, 64.0));
                        e.collider = Some(crate::entity::Collider2D::solid(32.0, 64.0));
                        e
                    }
                    "platform" => {
                        let mut e = crate::entity::Entity::new(next_id, format!("Plateforme_{:02}", next_id), "platform", crate::entity::Transform2D::new(x, y, 160.0, 24.0));
                        e.collider = Some(crate::entity::Collider2D::solid(160.0, 24.0));
                        e
                    }
                    "spikes" | "hazard" => {
                        let mut e = crate::entity::Entity::new(next_id, format!("Piques_{:02}", next_id), "hazard", crate::entity::Transform2D::new(x, y, 96.0, 20.0));
                        e.collider = Some(crate::entity::Collider2D::trigger(96.0, 20.0));
                        e
                    }
                    "crate" => {
                        let mut e = crate::entity::Entity::new(next_id, format!("Caisse_{:02}", next_id), "crate", crate::entity::Transform2D::new(x, y, 32.0, 32.0));
                        let mut c = crate::entity::Collider2D::solid(32.0, 32.0);
                        c.is_pushable = true;
                        e.collider = Some(c);
                        e
                    }
                    "plate" => {
                        let mut e = crate::entity::Entity::new(next_id, format!("Dalle_{:02}", next_id), "plate", crate::entity::Transform2D::new(x, y, 32.0, 32.0));
                        e.collider = Some(crate::entity::Collider2D::trigger(32.0, 32.0));
                        e
                    }
                    "boss" => {
                        let mut e = crate::entity::Entity::new(next_id, format!("Boss_{:02}", next_id), "boss", crate::entity::Transform2D::new(x, y, 72.0, 72.0));
                        e.collider = Some(crate::entity::Collider2D::solid(72.0, 72.0));
                        e.health = Some(crate::entity::Health2D::new(100.0));
                        e
                    }
                    _ => {
                        let mut e = crate::entity::Entity::new(next_id, format!("Mur_{:02}", next_id), "wall", crate::entity::Transform2D::new(x, y, 32.0, 32.0));
                        e.collider = Some(crate::entity::Collider2D::solid(32.0, 32.0));
                        e
                    }
                };
                r.game.entities.push(ent);
                r.game.set_status(format!("Entité #{} ('{}') générée avec succès.", next_id, preset));
                return next_id;
            }
        }
        0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_xrgb_to_rgba_fidelity() {
        // Source au format 0x00RRGGBB
        let src = [0x00FF0000, 0x0000FF00, 0x000000FF, 0x00123456, 0x00AABBCC];
        let mut dst = [0u32; 5];

        convert_xrgb_to_rgba(&src, &mut dst);

        // Vérification de la conversion en mémoire little-endian RGBA32 (0xFFBBGGRR)
        // 1. Rouge pur : [R=255, G=0, B=0, A=255] -> 0xFF0000FF
        assert_eq!(dst[0], 0xFF0000FF);
        // 2. Vert pur : [R=0, G=255, B=0, A=255] -> 0xFF00FF00
        assert_eq!(dst[1], 0xFF00FF00);
        // 3. Bleu pur : [R=0, G=0, B=255, A=255] -> 0xFFFF0000
        assert_eq!(dst[2], 0xFFFF0000);
        // 4. Mixte : R=0x12, G=0x34, B=0x56 -> 0xFF563412
        assert_eq!(dst[3], 0xFF563412);
        // 5. Reliquat (remainder chunk) : R=0xAA, G=0xBB, B=0xCC -> 0xFFCCBBAA
        assert_eq!(dst[4], 0xFFCCBBAA);
    }
}



