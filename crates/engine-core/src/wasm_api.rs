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
        _ => None,
    }
}

/// Initialise l'instance WebAssembly du moteur KaractiRage
#[wasm_bindgen]
pub fn wasm_init_engine() -> i32 {
    let game = match GameState::load_from_scene("assets/scenes/level_01.scene.json") {
        Ok(g) => g,
        Err(_) => return -1,
    };

    let canvas = Canvas::new(CANVAS_WIDTH, CANVAS_HEIGHT);
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
        let borrow = rt.borrow();
        if let Some(ref r) = *borrow {
            r.rgba_buffer.as_ptr()
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
            if let Some(ref mut r) = *rt.borrow_mut() {
                r.input.on_key_down(kc);
            }
        });
    }
}

/// Enregistre un relâchement touche depuis le navigateur
#[wasm_bindgen]
pub fn wasm_on_key_up(key: u32) {
    if let Some(kc) = web_key_to_keycode(key) {
        RUNTIME.with(|rt| {
            if let Some(ref mut r) = *rt.borrow_mut() {
                r.input.on_key_up(kc);
            }
        });
    }
}

/// Enregistre le déplacement de la souris sur le Canvas
#[wasm_bindgen]
pub fn wasm_on_mouse_move(x: f32, y: f32) {
    RUNTIME.with(|rt| {
        if let Some(ref mut r) = *rt.borrow_mut() {
            r.input.on_mouse_move(x, y);
        }
    });
}

/// Enregistre un clic de souris (bouton 0: gauche, bouton 2: droit)
#[wasm_bindgen]
pub fn wasm_on_mouse_down(button: u32) {
    RUNTIME.with(|rt| {
        if let Some(ref mut r) = *rt.borrow_mut() {
            if button == 0 {
                r.input.on_mouse_down();
            } else if button == 2 {
                r.input.on_mouse_right_click();
            }
        }
    });
}

/// Enregistre le relâchement du clic de souris
#[wasm_bindgen]
pub fn wasm_on_mouse_up(button: u32) {
    RUNTIME.with(|rt| {
        if let Some(ref mut r) = *rt.borrow_mut() {
            if button == 0 {
                r.input.on_mouse_up();
            }
        }
    });
}

/// Avance la simulation physique d'un pas dt et génère le rendu dans le buffer RGBA
#[wasm_bindgen]
pub fn wasm_render_frame(dt: f32) -> u32 {
    RUNTIME.with(|rt| {
        let mut borrow = rt.borrow_mut();
        if let Some(ref mut r) = *borrow {
            // 1. Mise à jour simulation & KVL
            r.game.update(dt, &r.input);

            // 2. Rendu logiciel dans le Canvas
            r.game.render(&mut r.canvas);

            // 3. Conversion du format softbuffer 0x00RRGGBB vers RGBA32 pour Canvas HTML5
            let src = &r.canvas.buffer;
            let dst = &mut r.rgba_buffer;
            let len = src.len().min(dst.len());

            for i in 0..len {
                let col = src[i];
                let r_val = (col >> 16) & 0xFF;
                let g_val = (col >> 8) & 0xFF;
                let b_val = col & 0xFF;
                // En mémoire little-endian : octet 0 = R, octet 1 = G, octet 2 = B, octet 3 = 255 (Alpha)
                dst[i] = r_val | (g_val << 8) | (b_val << 16) | 0xFF000000;
            }

            // 4. Nettoyage des événements transitoires d'entrée
            r.input.clear_transient();

            1
        } else {
            0
        }
    })
}

/// Récupère le score KVL actuel
#[wasm_bindgen]
pub fn wasm_get_score() -> i32 {
    RUNTIME.with(|rt| {
        let borrow = rt.borrow();
        if let Some(ref r) = *borrow {
            match r.game.kvl_context.get_var("score") {
                Some(kvl_core::Value::Int(s)) => *s as i32,
                _ => 0,
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
        let borrow = rt.borrow();
        if let Some(ref r) = *borrow {
            r.game.telemetry.current_fps
        } else {
            0.0
        }
    })
}

/// Récupère le temps moyen de calcul CPU par frame en microsecondes
#[wasm_bindgen]
pub fn wasm_get_cpu_us() -> u32 {
    RUNTIME.with(|rt| {
        let borrow = rt.borrow();
        if let Some(ref r) = *borrow {
            r.game.telemetry.avg_frame_time_us as u32
        } else {
            0
        }
    })
}

/// Récupère le nombre de requêtes spatiales de la dernière frame
#[wasm_bindgen]
pub fn wasm_get_spatial_queries() -> u32 {
    RUNTIME.with(|rt| {
        let borrow = rt.borrow();
        if let Some(ref r) = *borrow {
            r.game.spatial_grid.stats_queries as u32
        } else {
            0
        }
    })
}

/// Bascule la minicarte radar
#[wasm_bindgen]
pub fn wasm_toggle_radar() -> u32 {
    RUNTIME.with(|rt| {
        let mut borrow = rt.borrow_mut();
        if let Some(ref mut r) = *borrow {
            r.game.show_minimap = !r.game.show_minimap;
            if r.game.show_minimap { 1 } else { 0 }
        } else {
            0
        }
    })
}

/// Bascule la grille de partitionnement spatial
#[wasm_bindgen]
pub fn wasm_toggle_spatial_grid() -> u32 {
    RUNTIME.with(|rt| {
        let mut borrow = rt.borrow_mut();
        if let Some(ref mut r) = *borrow {
            let vis = r.game.spatial_grid.toggle_debug();
            if vis { 1 } else { 0 }
        } else {
            0
        }
    })
}

/// Bascule le mode éditeur KVL
#[wasm_bindgen]
pub fn wasm_toggle_editor_mode() -> u32 {
    RUNTIME.with(|rt| {
        let mut borrow = rt.borrow_mut();
        if let Some(ref mut r) = *borrow {
            r.game.editor.toggle_mode();
            match r.game.editor.mode {
                crate::editor::EditorMode::Edit => 1,
                crate::editor::EditorMode::Play => 0,
            }
        } else {
            0
        }
    })
}

/// Récupère le message de statut courant (UTF-8)
#[wasm_bindgen]
pub fn wasm_get_status_message() -> String {
    RUNTIME.with(|rt| {
        let borrow = rt.borrow();
        if let Some(ref r) = *borrow {
            r.game.status_message.clone()
        } else {
            String::new()
        }
    })
}
