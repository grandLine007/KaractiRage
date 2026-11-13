//! Cible WASM du moteur KaractiRage.
//!
//! Expose l'API consommée par `standalone_template.html` :
//! `wasm_init_engine` (bool : vrai si le moteur est prêt), `wasm_last_error`
//! (message de diagnostic), `wasm_import_game_package`, `wasm_render_frame`,
//! `wasm_get_framebuffer_ptr`, `wasm_get_canvas_width/height`, ainsi que les
//! entrées `wasm_on_key_down`, `wasm_on_key_up` et `wasm_on_mouse_move`.
//!
//! Build : `wasm-pack build crates/engine-wasm --out-dir ../../dist/pkg`
//! (ou `cargo build --target wasm32-unknown-unknown` + `wasm-bindgen`).
//! La scène par défaut est embarquée à la compilation (`include_str!`) : aucun accès
//! fichier à l'exécution ; l'audio se désactive proprement sans device. Aucun
//! chemin ne doit appeler `std::env::temp_dir()`/`Instant::now()` : ces fonctions
//! paniquent sous `wasm32-unknown-unknown` (voir `engine-core`).

#[cfg(target_arch = "wasm32")]
mod wasm_api {
    use std::sync::Mutex;
    use wasm_bindgen::prelude::*;

    static ENGINE: Mutex<Option<karactirage_engine::native_api::NativeEngine>> = Mutex::new(None);
    static FRAMEBUFFER: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    /// Dernier message d'échec d'initialisation (vide si aucune erreur).
    static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

    /// Initialise le moteur embarqué (scène `include_str!`, aucun accès disque).
    ///
    /// Renvoie `true` si le moteur est prêt, `false` sinon. Le nom et l'absence
    /// d'argument restent compatibles avec le template HTML, qui peut ignorer la
    /// valeur de retour ; un appelant qui veut afficher l'erreur interroge
    /// [`wasm_last_error`]. Contrairement à l'ancienne version, un échec n'est
    /// plus silencieux.
    #[wasm_bindgen]
    pub fn wasm_init_engine() -> bool {
        let mut engine = ENGINE.lock().unwrap();
        if engine.is_some() {
            return true;
        }
        match karactirage_engine::native_api::NativeEngine::init() {
            Ok(instance) => {
                *engine = Some(instance);
                *LAST_ERROR.lock().unwrap() = None;
                true
            }
            Err(message) => {
                *LAST_ERROR.lock().unwrap() = Some(message);
                false
            }
        }
    }

    /// Message d'erreur de la dernière initialisation (chaîne vide si succès).
    ///
    /// Le template peut l'afficher pour diagnostiquer un échec de démarrage sans
    /// dépendre de la console du navigateur.
    #[wasm_bindgen]
    pub fn wasm_last_error() -> String {
        LAST_ERROR
            .lock()
            .ok()
            .and_then(|error| error.clone())
            .unwrap_or_default()
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

    /// Transmet un appui clavier au moteur.
    ///
    /// `key` est un code WebKey de la table partagée avec le Studio (0 =
    /// flèche gauche, 1 = flèche droite, …, 40 = Y). Le moteur ignore
    /// silencieusement les codes inconnus : aucune touche ne peut donc
    /// interrompre la boucle de rendu.
    #[wasm_bindgen]
    pub fn wasm_on_key_down(key: u32) {
        let mut engine = ENGINE.lock().unwrap();
        if let Some(e) = engine.as_mut() {
            e.on_key_down(key);
        }
    }

    /// Transmet un relâchement clavier au moteur (même table que
    /// [`wasm_on_key_down`]).
    #[wasm_bindgen]
    pub fn wasm_on_key_up(key: u32) {
        let mut engine = ENGINE.lock().unwrap();
        if let Some(e) = engine.as_mut() {
            e.on_key_up(key);
        }
    }

    /// Transmet la position de la souris au moteur, en coordonnées internes
    /// du canvas (1200 × 800). Sans moteur, l'appel est sans effet.
    #[wasm_bindgen]
    pub fn wasm_on_mouse_move(x: f32, y: f32) {
        let mut engine = ENGINE.lock().unwrap();
        if let Some(e) = engine.as_mut() {
            e.on_mouse_move(x, y);
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
