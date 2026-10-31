//! Cible WASM du moteur KaractiRage.
//!
//! Expose l'API consommée par `standalone_template.html` :
//! `wasm_init_engine`, `wasm_import_game_package`, `wasm_render_frame`,
//! `wasm_get_framebuffer_ptr`, `wasm_get_canvas_width/height`.
//!
//! Build : `wasm-pack build crates/engine-wasm --out-dir ../../dist/pkg`
//! (ou `cargo build --target wasm32-unknown-unknown` + `wasm-bindgen`).
//! La scène par défaut est embarquée à la compilation (`include_str!`) : aucun accès
//! fichier à l'exécution ; l'audio se désactive proprement sans device.

#[cfg(target_arch = "wasm32")]
mod wasm_api {
    use std::sync::Mutex;
    use wasm_bindgen::prelude::*;

    static ENGINE: Mutex<Option<karactirage_engine::native_api::NativeEngine>> = Mutex::new(None);
    static FRAMEBUFFER: Mutex<Vec<u8>> = Mutex::new(Vec::new());

    #[wasm_bindgen]
    pub fn wasm_init_engine() {
        let mut engine = ENGINE.lock().unwrap();
        if engine.is_none() {
            *engine = karactirage_engine::native_api::NativeEngine::init().ok();
        }
    }

    #[wasm_bindgen]
    pub fn wasm_import_game_package(json: String) {
        let mut engine = ENGINE.lock().unwrap();
        if let Some(e) = engine.as_mut() {
            e.import_game_package(&json);
        }
    }

    #[wasm_bindgen]
    pub fn wasm_render_frame(dt: f32) {
        let mut engine = ENGINE.lock().unwrap();
        if let Some(e) = engine.as_mut() {
            let bytes = e.render_frame(dt);
            *FRAMEBUFFER.lock().unwrap() = bytes;
        }
    }

    #[wasm_bindgen]
    pub fn wasm_get_framebuffer_ptr() -> *const u8 {
        FRAMEBUFFER.lock().unwrap().as_ptr()
    }

    #[wasm_bindgen]
    pub fn wasm_get_canvas_width() -> u32 {
        karactirage_engine::native_api::CANVAS_WIDTH as u32
    }

    #[wasm_bindgen]
    pub fn wasm_get_canvas_height() -> u32 {
        karactirage_engine::native_api::CANVAS_HEIGHT as u32
    }

    /// Expose le `WebAssembly.Memory` (nécessaire avec `--target web`, qui n'exporte
    /// pas `__wbindgen_memory`).
    #[wasm_bindgen]
    pub fn wasm_memory() -> JsValue {
        wasm_bindgen::memory()
    }
}
