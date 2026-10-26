use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tauri::Manager;
use karactirage_engine::ai_client::{AiClient, AiClientConfig, AiProvider};
use karactirage_engine::native_api::NativeEngine;
use karactirage_engine::project_registry::{NewProjectConfig, ProjectRegistry};

/// Moteur partagé à initialisation **différée** : la fenêtre s'ouvre immédiatement
/// pendant que `NativeEngine::init()` tourne sur un thread dédié. `lock()` attend la
/// fin du boot (bornée par un timeout) et reproduit l'API de `Mutex::lock`, ce qui
/// permet de garder inchangés tous les appels `engine.lock().unwrap()` existants.
type SharedEngine = Arc<EngineState>;
type EngineLockError = String;

pub struct EngineState {
    engine: Mutex<Option<NativeEngine>>,
    error: Mutex<Option<String>>,
    ready: Condvar,
    init_ms: AtomicU64,
    booting: AtomicBool,
}

pub struct EngineGuard<'a> {
    guard: MutexGuard<'a, Option<NativeEngine>>,
}

impl Deref for EngineGuard<'_> {
    type Target = NativeEngine;
    fn deref(&self) -> &NativeEngine {
        self.guard.as_ref().expect("moteur initialisé")
    }
}

impl DerefMut for EngineGuard<'_> {
    fn deref_mut(&mut self) -> &mut NativeEngine {
        self.guard.as_mut().expect("moteur initialisé")
    }
}

impl EngineState {
    fn new() -> Self {
        Self {
            engine: Mutex::new(None),
            error: Mutex::new(None),
            ready: Condvar::new(),
            init_ms: AtomicU64::new(0),
            booting: AtomicBool::new(false),
        }
    }

    /// Réserve le slot de boot : une seule initialisation à la fois (garde contre
    /// les doubles relances). Retourne `false` si un boot est déjà en cours ou fait.
    fn begin_boot(&self) -> bool {
        self.booting
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Consigne le résultat du boot et réveille les commandes en attente.
    fn finish_boot(&self, started: Instant, result: Result<NativeEngine, String>) {
        self.init_ms.store(started.elapsed().as_millis() as u64, Ordering::Relaxed);
        let mut slot = self.engine.lock().unwrap();
        match result {
            Ok(e) => *slot = Some(e),
            Err(e) => *self.error.lock().unwrap() = Some(e),
        }
        drop(slot);
        self.booting.store(false, Ordering::Release);
        self.ready.notify_all();
    }

    /// Attend la fin du boot puis expose le moteur (même API que `Mutex::lock`).
    pub fn lock(&self) -> Result<EngineGuard<'_>, EngineLockError> {
        let mut guard = self.engine.lock().unwrap_or_else(|e| e.into_inner());
        let deadline = Instant::now() + Duration::from_secs(10);
        while guard.is_none() {
            if let Some(err) = self.error.lock().unwrap().as_ref() {
                return Err(err.clone());
            }
            if Instant::now() >= deadline {
                return Err("Délai d'initialisation du moteur dépassé".to_string());
            }
            let (g, _) = self
                .ready
                .wait_timeout(guard, Duration::from_millis(100))
                .unwrap_or_else(|e| e.into_inner());
            guard = g;
        }
        Ok(EngineGuard { guard })
    }

    /// État de boot non bloquant (pour l'écran de démarrage côté Studio).
    fn status(&self) -> EngineBootStatus {
        let error = self.error.lock().unwrap().clone();
        let ready = self.engine.lock().unwrap().is_some();
        EngineBootStatus {
            state: if ready {
                "ready".to_string()
            } else if error.is_some() {
                "failed".to_string()
            } else {
                "booting".to_string()
            },
            error,
            init_ms: self.init_ms.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EngineInfo {
    pub name: String,
    pub version: String,
    pub architecture: String,
    pub default_llm_endpoint: String,
    pub default_llm_model: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LlmTestResult {
    pub success: bool,
    pub message: String,
    pub provider: String,
    pub endpoint: String,
    pub detected_model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AiGenerationResult {
    pub success: bool,
    pub message: String,
    pub payload_json: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerPosition {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EngineTelemetry {
    pub fps: f32,
    pub cpu_us: u32,
    pub score: i32,
    pub status: String,
    pub update_us: u64,
    pub render_us: u64,
    pub encode_us: u64,
    pub total_us: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EngineBootStatus {
    pub state: String,
    pub error: Option<String>,
    pub init_ms: u64,
}

#[tauri::command]
fn get_engine_info() -> EngineInfo {
    EngineInfo {
        name: "KaractiRage Studio".to_string(),
        version: "0.1.0 (Tauri v2 + React)".to_string(),
        architecture: "Hybrid Rust Wasm 60 FPS + React Studio".to_string(),
        default_llm_endpoint: "http://127.0.0.1:1234".to_string(),
        default_llm_model: "nvidia/nemotron-3-nano-4b".to_string(),
    }
}

#[tauri::command]
fn engine_status(engine: tauri::State<'_, SharedEngine>) -> EngineBootStatus {
    engine.status()
}

#[tauri::command]
fn engine_retry_boot(engine: tauri::State<'_, SharedEngine>) -> bool {
    if engine.engine.lock().unwrap().is_some() {
        return false;
    }
    // Une seule initialisation à la fois (garde contre les doubles relances).
    if !engine.begin_boot() {
        return false;
    }
    // Efface l'ancienne erreur AVANT de relancer : les commandes en attente doivent
    // patienter le temps du boot, pas échouer immédiatement sur l'erreur périmée.
    *engine.error.lock().unwrap() = None;
    let state = engine.inner().clone();
    std::thread::spawn(move || {
        let started = Instant::now();
        let result = NativeEngine::init();
        state.finish_boot(started, result);
    });
    true
}

#[tauri::command]
fn test_llm_connection(provider_name: String, endpoint: Option<String>) -> LlmTestResult {
    let provider = match provider_name.to_lowercase().as_str() {
        "lmstudio" | "lm studio" => AiProvider::LmStudio,
        "ollama" => AiProvider::Ollama,
        "claude" => AiProvider::Claude,
        "gemini" => AiProvider::Gemini,
        "deepseek" => AiProvider::DeepSeek,
        "openai" => AiProvider::OpenAI,
        _ => AiProvider::OfflineLocal,
    };

    let mut config = AiClientConfig::load_or_default();
    config.provider = provider;
    if let Some(ep) = endpoint {
        if !ep.trim().is_empty() {
            config.custom_endpoint = Some(ep);
        }
    }

    let ep_str = config.custom_endpoint.clone().unwrap_or_else(|| provider.default_endpoint().to_string());

    match AiClient::test_connection(&config) {
        Ok(msg) => LlmTestResult {
            success: true,
            message: msg,
            provider: provider.label().to_string(),
            endpoint: ep_str,
            detected_model: Some("nvidia/nemotron-3-nano-4b".to_string()),
        },
        Err(err) => LlmTestResult {
            success: false,
            message: err,
            provider: provider.label().to_string(),
            endpoint: ep_str,
            detected_model: None,
        },
    }
}

#[tauri::command]
fn generate_ai_level(prompt: String, provider_name: Option<String>, endpoint: Option<String>) -> AiGenerationResult {
    let mut config = AiClientConfig::load_or_default();
    if let Some(p_name) = provider_name {
        config.provider = match p_name.to_lowercase().as_str() {
            "lmstudio" | "lm studio" => AiProvider::LmStudio,
            "ollama" => AiProvider::Ollama,
            "claude" => AiProvider::Claude,
            "gemini" => AiProvider::Gemini,
            "deepseek" => AiProvider::DeepSeek,
            "openai" => AiProvider::OpenAI,
            _ => AiProvider::OfflineLocal,
        };
    }
    if let Some(ep) = endpoint {
        if !ep.trim().is_empty() {
            config.custom_endpoint = Some(ep);
        }
    }

    match AiClient::send_prompt(&config, &prompt) {
        Ok(payload) => {
            let json = serde_json::to_string_pretty(&payload).unwrap_or_default();
            AiGenerationResult {
                success: true,
                message: format!("Niveau généré avec succès : {}", payload.title),
                payload_json: Some(json),
            }
        }
        Err(err) => AiGenerationResult {
            success: false,
            message: err,
            payload_json: None,
        },
    }
}

// ============================================================
// Commandes du moteur natif embarqué (un seul moteur, pas de Wasm)
// ============================================================

#[tauri::command]
fn engine_frame(dt: f32, engine: tauri::State<'_, SharedEngine>) -> tauri::ipc::Response {
    let mut eng = engine.lock().unwrap();
    let bytes = eng.render_frame(dt);
    tauri::ipc::Response::new(bytes)
}

#[tauri::command]
fn engine_frame_packet(dt: f32, engine: tauri::State<'_, SharedEngine>) -> tauri::ipc::Response {
    let mut eng = engine.lock().unwrap();
    let bytes = eng.render_packet_frame(dt);
    tauri::ipc::Response::new(bytes)
}

#[tauri::command]
fn engine_list_mesh_assets() -> String {
    use karactirage_engine::mesh_assets::{MeshAssetKind, MeshManifest};
    let manifest = MeshManifest::builtin();
    let summary: Vec<serde_json::Value> = manifest
        .assets
        .iter()
        .map(|a| {
            let (kind, parts) = match &a.kind {
                MeshAssetKind::Procedural { parts } => ("procedural", parts.len()),
                MeshAssetKind::Gltf { .. } => ("gltf", 1),
            };
            serde_json::json!({ "id": a.id, "kind": kind, "parts": parts })
        })
        .collect();
    serde_json::to_string(&summary).unwrap_or_else(|_| "[]".to_string())
}

#[tauri::command]
fn engine_spawn_3d_preset(preset: String, x: f32, y: f32, z: f32, engine: tauri::State<'_, SharedEngine>) -> usize {
    engine.lock().unwrap().spawn_3d_preset(&preset, x, y, z)
}

#[tauri::command]
fn engine_new_3d_scene(engine: tauri::State<'_, SharedEngine>) -> usize {
    let mut eng = engine.lock().unwrap();
    // Retire les entités 3D existantes, puis repeuple une scène par défaut.
    eng.game.entities.retain(|e| e.transform3d.is_none());
    eng.populate_default_3d_scene();
    eng.game
        .entities
        .iter()
        .filter(|e| e.transform3d.is_some())
        .count()
}

#[tauri::command]
fn engine_move_entity_3d(id: usize, x: f32, z: f32, engine: tauri::State<'_, SharedEngine>) -> bool {
    engine.lock().unwrap().move_entity_3d(id, x, z)
}

#[tauri::command]
fn engine_canvas_width(engine: tauri::State<'_, SharedEngine>) -> u32 {
    engine.lock().unwrap().canvas_width()
}

#[tauri::command]
fn engine_canvas_height(engine: tauri::State<'_, SharedEngine>) -> u32 {
    engine.lock().unwrap().canvas_height()
}

#[tauri::command]
fn engine_on_key_down(key: u32, engine: tauri::State<'_, SharedEngine>) {
    engine.lock().unwrap().on_key_down(key);
}

#[tauri::command]
fn engine_on_key_up(key: u32, engine: tauri::State<'_, SharedEngine>) {
    engine.lock().unwrap().on_key_up(key);
}

#[tauri::command]
fn engine_on_mouse_move(x: f32, y: f32, engine: tauri::State<'_, SharedEngine>) {
    engine.lock().unwrap().on_mouse_move(x, y);
}

#[tauri::command]
fn engine_on_mouse_down(button: u32, engine: tauri::State<'_, SharedEngine>) {
    engine.lock().unwrap().on_mouse_down(button);
}

#[tauri::command]
fn engine_on_mouse_up(button: u32, engine: tauri::State<'_, SharedEngine>) {
    engine.lock().unwrap().on_mouse_up(button);
}

#[tauri::command]
fn engine_get_score(engine: tauri::State<'_, SharedEngine>) -> i32 {
    engine.lock().unwrap().get_score()
}

#[tauri::command]
fn engine_get_fps(engine: tauri::State<'_, SharedEngine>) -> f32 {
    engine.lock().unwrap().get_fps()
}

#[tauri::command]
fn engine_get_cpu_us(engine: tauri::State<'_, SharedEngine>) -> u32 {
    engine.lock().unwrap().get_cpu_us()
}

#[tauri::command]
fn engine_get_status_message(engine: tauri::State<'_, SharedEngine>) -> String {
    engine.lock().unwrap().get_status_message()
}

#[tauri::command]
fn engine_telemetry(engine: tauri::State<'_, SharedEngine>) -> EngineTelemetry {
    let eng = engine.lock().unwrap();
    let s = eng.frame_stats;
    EngineTelemetry {
        fps: eng.get_fps(),
        cpu_us: s.total_us as u32,
        score: eng.get_score(),
        status: eng.get_status_message(),
        update_us: s.update_us,
        render_us: s.render_us,
        encode_us: s.encode_us,
        total_us: s.total_us,
    }
}

#[tauri::command]
fn engine_toggle_radar(engine: tauri::State<'_, SharedEngine>) -> u32 {
    engine.lock().unwrap().toggle_radar()
}

#[tauri::command]
fn engine_toggle_spatial_grid(engine: tauri::State<'_, SharedEngine>) -> u32 {
    engine.lock().unwrap().toggle_spatial_grid()
}

#[tauri::command]
fn engine_toggle_editor_mode(engine: tauri::State<'_, SharedEngine>) -> u32 {
    engine.lock().unwrap().toggle_editor_mode()
}

#[tauri::command]
fn engine_set_embedded_mode(enabled: bool, engine: tauri::State<'_, SharedEngine>) {
    engine.lock().unwrap().set_embedded_mode(enabled);
}

#[tauri::command]
fn engine_validate_recipe(recipe_json: String, engine: tauri::State<'_, SharedEngine>) -> String {
    let _ = engine;
    NativeEngine::validate_recipe(&recipe_json)
}

#[tauri::command]
fn engine_apply_recipe(recipe_json: String, engine: tauri::State<'_, SharedEngine>) -> i32 {
    engine.lock().unwrap().apply_recipe(&recipe_json)
}

#[tauri::command]
fn engine_launch_archetype(
    archetype_id: String,
    engine: tauri::State<'_, SharedEngine>,
) -> Result<String, String> {
    engine.lock().unwrap().launch_default_game(&archetype_id)
}

#[tauri::command]
fn engine_launch_archetype_3d(
    archetype_id: String,
    engine: tauri::State<'_, SharedEngine>,
) -> Result<usize, String> {
    let mut eng = engine.lock().unwrap();
    eng.populate_archetype_3d_scene(&archetype_id)
}

#[tauri::command]
fn engine_apply_ai_payload(
    payload_json: String,
    prompt: String,
    engine: tauri::State<'_, SharedEngine>,
) -> i32 {
    engine.lock().unwrap().apply_ai_payload(&payload_json, &prompt)
}

#[tauri::command]
fn engine_accept_ai_patch(engine: tauri::State<'_, SharedEngine>) -> i32 {
    engine.lock().unwrap().accept_ai_patch()
}

#[tauri::command]
fn engine_reject_ai_patch(engine: tauri::State<'_, SharedEngine>) -> i32 {
    engine.lock().unwrap().reject_ai_patch()
}

#[tauri::command]
fn engine_has_pending_ai_patch(engine: tauri::State<'_, SharedEngine>) -> i32 {
    engine.lock().unwrap().has_pending_ai_patch()
}

#[tauri::command]
fn engine_set_active_tool(tool: u32, engine: tauri::State<'_, SharedEngine>) -> u32 {
    engine.lock().unwrap().set_active_tool(tool)
}

#[tauri::command]
fn engine_get_active_tool(engine: tauri::State<'_, SharedEngine>) -> u32 {
    engine.lock().unwrap().get_active_tool()
}

#[tauri::command]
fn engine_cycle_brush_tile(engine: tauri::State<'_, SharedEngine>) -> String {
    engine.lock().unwrap().cycle_brush_tile()
}

#[tauri::command]
fn engine_get_active_brush_tile(engine: tauri::State<'_, SharedEngine>) -> String {
    engine.lock().unwrap().get_active_brush_tile()
}

#[tauri::command]
fn engine_export_standalone_html(title: String, author: String, engine: tauri::State<'_, SharedEngine>) -> String {
    engine.lock().unwrap().export_standalone_html(&title, &author)
}

#[tauri::command]
fn engine_move_player_to(x: f32, y: f32, engine: tauri::State<'_, SharedEngine>) -> bool {
    engine.lock().unwrap().move_player_to(x, y)
}

#[tauri::command]
fn engine_get_player_pos(engine: tauri::State<'_, SharedEngine>) -> PlayerPosition {
    let (x, y) = engine.lock().unwrap().get_player_pos();
    PlayerPosition { x, y }
}

#[tauri::command]
fn engine_interact(engine: tauri::State<'_, SharedEngine>) -> bool {
    engine.lock().unwrap().interact()
}

#[tauri::command]
fn engine_new_blank_project(title: String, genre: String, engine: tauri::State<'_, SharedEngine>) -> i32 {
    engine.lock().unwrap().new_blank_project(&title, &genre)
}

#[tauri::command]
fn engine_load_project_json(project_json: String, engine: tauri::State<'_, SharedEngine>) -> i32 {
    engine.lock().unwrap().load_project_json(&project_json)
}

#[tauri::command]
fn engine_export_project_json(engine: tauri::State<'_, SharedEngine>) -> String {
    engine.lock().unwrap().export_project_json()
}

#[tauri::command]
fn engine_get_entities_summary(engine: tauri::State<'_, SharedEngine>) -> String {
    engine.lock().unwrap().get_entities_summary_json()
}

#[tauri::command]
fn engine_update_entity_transform(
    id: usize,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    record_history: bool,
    engine: tauri::State<'_, SharedEngine>,
) -> bool {
    engine
        .lock()
        .unwrap()
        .update_entity_transform(id, x, y, w, h, record_history)
}

#[tauri::command]
fn engine_delete_entity(id: usize, engine: tauri::State<'_, SharedEngine>) -> bool {
    engine.lock().unwrap().delete_entity(id)
}

#[tauri::command]
fn engine_spawn_entity_preset(preset: String, x: f32, y: f32, engine: tauri::State<'_, SharedEngine>) -> usize {
    engine.lock().unwrap().spawn_entity_preset(&preset, x, y)
}

#[tauri::command]
fn engine_editor_state(engine: tauri::State<'_, SharedEngine>) -> String {
    engine.lock().unwrap().editor_state_json()
}

#[tauri::command]
fn engine_select_entity(id: usize, engine: tauri::State<'_, SharedEngine>) -> bool {
    engine.lock().unwrap().select_entity(id)
}

#[tauri::command]
fn engine_get_selected_entity(engine: tauri::State<'_, SharedEngine>) -> Option<usize> {
    engine.lock().unwrap().get_selected_entity_id()
}

#[tauri::command]
fn engine_delete_selected_entity(engine: tauri::State<'_, SharedEngine>) -> bool {
    engine.lock().unwrap().delete_selected_entity()
}

#[tauri::command]
fn engine_duplicate_selected_entity(engine: tauri::State<'_, SharedEngine>) -> Option<usize> {
    engine.lock().unwrap().duplicate_selected_entity()
}

#[tauri::command]
fn engine_undo(engine: tauri::State<'_, SharedEngine>) -> bool {
    engine.lock().unwrap().undo()
}

#[tauri::command]
fn engine_redo(engine: tauri::State<'_, SharedEngine>) -> bool {
    engine.lock().unwrap().redo()
}

#[tauri::command]
fn engine_set_editor_mode(mode: u32, engine: tauri::State<'_, SharedEngine>) -> u32 {
    engine.lock().unwrap().set_editor_mode(mode)
}

#[tauri::command]
fn engine_set_brush_tile(tile_id: String, engine: tauri::State<'_, SharedEngine>) -> String {
    engine.lock().unwrap().set_brush_tile_by_id(&tile_id)
}

#[tauri::command]
fn engine_get_brush_tiles(engine: tauri::State<'_, SharedEngine>) -> String {
    engine.lock().unwrap().get_brush_tiles_json()
}

#[tauri::command]
fn engine_spawn_entity_at_screen(preset: String, sx: f32, sy: f32, engine: tauri::State<'_, SharedEngine>) -> usize {
    engine.lock().unwrap().spawn_preset_at_screen(&preset, sx, sy)
}

#[tauri::command]
fn engine_save_scene(engine: tauri::State<'_, SharedEngine>) -> bool {
    engine.lock().unwrap().save_scene()
}

#[tauri::command]
fn engine_list_sprite_assets(engine: tauri::State<'_, SharedEngine>) -> String {
    engine.lock().unwrap().list_sprite_assets_json()
}

#[tauri::command]
fn engine_sprite_png(id: String, engine: tauri::State<'_, SharedEngine>) -> tauri::ipc::Response {
    match engine.lock().unwrap().sprite_png_bytes(&id) {
        Some(bytes) => tauri::ipc::Response::new(bytes),
        None => tauri::ipc::Response::new(Vec::<u8>::new()),
    }
}

#[tauri::command]
fn engine_list_anim_assets(engine: tauri::State<'_, SharedEngine>) -> String {
    engine.lock().unwrap().list_anim_assets_json()
}

#[tauri::command]
fn engine_list_sheet_assets(engine: tauri::State<'_, SharedEngine>) -> String {
    engine.lock().unwrap().list_sheet_assets_json()
}

#[tauri::command]
fn engine_sheet_png(id: String, engine: tauri::State<'_, SharedEngine>) -> tauri::ipc::Response {
    match engine.lock().unwrap().sheet_png_bytes(&id) {
        Some(bytes) => tauri::ipc::Response::new(bytes),
        None => tauri::ipc::Response::new(Vec::<u8>::new()),
    }
}

#[tauri::command]
fn engine_import_raster(id: String, png_base64: String, engine: tauri::State<'_, SharedEngine>) -> Result<(u32, u32), String> {
    engine.lock().unwrap().import_raster_png(&id, &png_base64)
}

#[tauri::command]
fn engine_import_svg(id: String, svg_text: String, engine: tauri::State<'_, SharedEngine>) -> Result<(u32, u32), String> {
    engine.lock().unwrap().import_svg(&id, &svg_text)
}

#[tauri::command]
fn engine_vectorize_raster(id: String, png_base64: String, opts_json: String, engine: tauri::State<'_, SharedEngine>) -> Result<String, String> {
    engine.lock().unwrap().vectorize_raster_asset(&id, &png_base64, &opts_json)
}

#[tauri::command]
fn engine_list_vector_assets(engine: tauri::State<'_, SharedEngine>) -> String {
    engine.lock().unwrap().list_vector_assets_json()
}

#[tauri::command]
fn engine_vector_load(id: String, engine: tauri::State<'_, SharedEngine>) -> Result<String, String> {
    engine.lock().unwrap().vector_load_json(&id)
}

#[tauri::command]
fn engine_vector_save(id: String, model_json: String, engine: tauri::State<'_, SharedEngine>) -> Result<String, String> {
    engine.lock().unwrap().vector_save(&id, &model_json)
}

#[tauri::command]
fn engine_vector_spawn(id: String, engine: tauri::State<'_, SharedEngine>) -> Result<usize, String> {
    engine.lock().unwrap().spawn_vector_asset(&id)
}

#[tauri::command]
fn engine_get_kvl_graphs(engine: tauri::State<'_, SharedEngine>) -> String {
    engine.lock().unwrap().get_kvl_graphs_json()
}

#[tauri::command]
fn engine_get_kvl_graph(path: String, engine: tauri::State<'_, SharedEngine>) -> String {
    engine.lock().unwrap().get_kvl_graph_json(&path)
}

#[tauri::command]
fn engine_validate_kvl_graph(json: String, engine: tauri::State<'_, SharedEngine>) -> String {
    engine.lock().unwrap().validate_kvl_graph_json(&json)
}

#[tauri::command]
fn engine_save_kvl_graph(path: String, json: String, engine: tauri::State<'_, SharedEngine>) -> bool {
    engine.lock().unwrap().save_kvl_graph(&path, &json)
}

#[tauri::command]
fn engine_build_guided_graph(recipe: String, params: serde_json::Value, engine: tauri::State<'_, SharedEngine>) -> String {
    engine.lock().unwrap().build_guided_graph(&recipe, params)
}

#[tauri::command]
fn engine_attach_script(entity_id: usize, script_path: String, engine: tauri::State<'_, SharedEngine>) -> bool {
    engine.lock().unwrap().attach_script_to_entity(entity_id, &script_path)
}

// ---------------------------------------------------------------------------
// Hub de Projets Unifié — découverte, création et gestion sur disque (Section 1.2)
// ---------------------------------------------------------------------------

#[tauri::command]
fn engine_list_projects(extra_dirs: Vec<String>) -> Result<String, String> {
    let registry = ProjectRegistry::open();
    let mut entries = registry.scan_default_sources();
    if !extra_dirs.is_empty() {
        let dirs: Vec<std::path::PathBuf> = extra_dirs.iter().map(std::path::PathBuf::from).collect();
        let extra = registry.scan(&dirs);
        let mut known: std::collections::HashSet<String> =
            entries.iter().map(|e| e.path.clone()).collect();
        for e in extra {
            if known.insert(e.path.clone()) {
                entries.push(e);
            }
        }
        entries.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    }
    serde_json::to_string(&entries).map_err(|e| e.to_string())
}

#[tauri::command]
fn engine_create_project(
    config_json: String,
    engine: tauri::State<'_, SharedEngine>,
) -> Result<String, String> {
    let config: NewProjectConfig =
        serde_json::from_str(&config_json).map_err(|e| format!("Config invalide : {}", e))?;
    let mut registry = ProjectRegistry::open();
    let entry = registry.create_project(&config)?;
    // Charge immédiatement le projet fraîchement créé dans le moteur.
    let mut eng = engine.lock().unwrap();
    if let Err(e) = eng.open_project_path(&entry.path) {
        return Err(format!("Projet créé mais chargement échoué : {}", e));
    }
    serde_json::to_string(&entry).map_err(|e| e.to_string())
}

#[tauri::command]
fn engine_open_project(path: String, engine: tauri::State<'_, SharedEngine>) -> Result<i32, String> {
    let mut registry = ProjectRegistry::open();
    registry.record_recent(&path);
    let _ = registry.save();
    engine.lock().unwrap().open_project_path(&path)
}

#[tauri::command]
fn engine_recent_projects() -> String {
    let registry = ProjectRegistry::open();
    serde_json::to_string(&registry.recent).unwrap_or_else(|_| "[]".to_string())
}

#[tauri::command]
fn engine_remove_recent(path: String) -> bool {
    let mut registry = ProjectRegistry::open();
    let removed = registry.remove_recent(&path);
    let _ = registry.save();
    removed
}

#[tauri::command]
fn engine_duplicate_project(path: String, new_title: String) -> Result<String, String> {
    let mut registry = ProjectRegistry::open();
    let entry = registry.duplicate_project(&path, &new_title)?;
    serde_json::to_string(&entry).map_err(|e| e.to_string())
}

#[tauri::command]
fn engine_rename_project(path: String, new_title: String) -> Result<String, String> {
    let mut registry = ProjectRegistry::open();
    let entry = registry.rename_project(&path, &new_title)?;
    serde_json::to_string(&entry).map_err(|e| e.to_string())
}

#[tauri::command]
fn engine_archive_project(path: String) -> Result<String, String> {
    let mut registry = ProjectRegistry::open();
    let archived = registry.archive_project(&path)?;
    Ok(archived.display().to_string())
}

#[tauri::command]
fn engine_reveal_project(path: String) -> Result<(), String> {
    if !std::path::Path::new(&path).exists() {
        return Err("Le fichier est introuvable.".to_string());
    }
    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").arg("-R").arg(&path).status();
    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("explorer")
        .arg("/select,")
        .arg(&path)
        .status();
    #[cfg(target_os = "linux")]
    let status = {
        let dir = std::path::Path::new(&path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
        std::process::Command::new("xdg-open").arg(dir).status()
    };
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(_) => Err("Le gestionnaire de fichiers a échoué.".to_string()),
        Err(e) => Err(format!("Impossible d'ouvrir le dossier : {}", e)),
    }
}

#[tauri::command]
fn engine_system_diagnostics(engine: tauri::State<'_, SharedEngine>) -> String {
    let eng = engine.lock().unwrap();
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    serde_json::json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "cpu_cores": cores,
        "fps": eng.get_fps(),
        "entities": eng.game.entities.len(),
        "projects_home": ProjectRegistry::home_dir().display().to_string(),
        "engine": "KaractiRage Engine Core 0.1.0",
    })
    .to_string()
}

/// Processus `pxl serve` actif (tué à l'arrêt explicite ou à la fermeture).
struct PxlServe(Mutex<Option<Child>>);

/// Racine du dépôt, indépendante du cwd (`apps/studio/src-tauri`).
fn pxl_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Résout `assets/art/{name}.px.js` en chemin absolu, en interdisant la
/// traversée de répertoires (nom = lettres/chiffres/`_`/`-` uniquement).
fn resolve_art_file(name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err("Nom d'art invalide (lettres, chiffres, _ et - uniquement).".to_string());
    }
    let file = pxl_root().join("assets/art").join(format!("{name}.px.js"));
    if !file.exists() {
        return Err(format!("Fichier d'art introuvable : {}", file.display()));
    }
    Ok(file)
}

/// Résout `assets/art/{name}.px.js` en chemin absolu **sans exiger l'existence**
/// (pour la création/écriture). Même validation de nom que `resolve_art_file`.
fn art_file_path(name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err("Nom d'art invalide (lettres, chiffres, _ et - uniquement).".to_string());
    }
    Ok(pxl_root().join("assets/art").join(format!("{name}.px.js")))
}

#[derive(Serialize)]
struct PxlBuildResult {
    ok: bool,
    stdout: String,
    stderr: String,
    duration_ms: u64,
}

#[derive(Serialize)]
struct PxlServeStatus {
    running: bool,
    url: Option<String>,
    port: u16,
}

#[tauri::command]
fn pxl_available() -> bool {
    if !pxl_root().join("tools/pxl/run.js").exists() {
        return false;
    }
    Command::new("node")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tauri::command]
fn pxl_build(name: String) -> Result<PxlBuildResult, String> {
    let file = resolve_art_file(&name)?;
    let run_js = pxl_root().join("tools/pxl/run.js");
    let out_dir = pxl_root().join("assets/generated");
    let t0 = std::time::Instant::now();
    let out = Command::new("node")
        .arg(&run_js)
        .arg("build")
        .arg(&file)
        .arg("--out")
        .arg(&out_dir)
        .output()
        .map_err(|e| format!("Impossible de lancer node : {e}"))?;
    Ok(PxlBuildResult {
        ok: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        duration_ms: t0.elapsed().as_millis() as u64,
    })
}

#[tauri::command]
fn pxl_serve_start(name: String, port: u16, state: tauri::State<'_, PxlServe>) -> Result<String, String> {
    let file = resolve_art_file(&name)?;
    let run_js = pxl_root().join("tools/pxl/run.js");
    let out_dir = pxl_root().join("assets/generated");

    // Stoppe un éventuel serveur précédent avant d'en relancer un.
    if let Some(mut child) = state.0.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
    }

    let child = Command::new("node")
        .arg(&run_js)
        .arg("serve")
        .arg(&file)
        .arg("--port")
        .arg(port.to_string())
        .arg("--out")
        .arg(&out_dir)
        .spawn()
        .map_err(|e| format!("Échec du lancement de pxl serve : {e}"))?;

    *state.0.lock().unwrap() = Some(child);
    Ok(format!("http://127.0.0.1:{port}/"))
}

#[tauri::command]
fn pxl_serve_stop(state: tauri::State<'_, PxlServe>) -> bool {
    if let Some(mut child) = state.0.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
        return true;
    }
    false
}

#[tauri::command]
fn pxl_serve_status(port: u16, state: tauri::State<'_, PxlServe>) -> PxlServeStatus {
    let mut guard = state.0.lock().unwrap();
    let mut running = false;
    let mut dead = false;
    if let Some(child) = guard.as_mut() {
        match child.try_wait() {
            Ok(None) => running = true,
            _ => dead = true,
        }
    }
    if dead {
        *guard = None;
    }
    PxlServeStatus {
        running,
        url: running.then(|| format!("http://127.0.0.1:{port}/")),
        port,
    }
}

/// Liste les sources d'art pxl disponibles (`assets/art/*.px.js`), triées.
#[tauri::command]
fn pxl_list_sources() -> Vec<String> {
    let dir = pxl_root().join("assets/art");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let p = e.path();
                    if p.extension().and_then(|x| x.to_str()) == Some("js") {
                        p.file_stem().and_then(|s| s.to_str()).map(str::to_string)
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Lit le contenu d'une source d'art pxl (fichier existant uniquement).
#[tauri::command]
fn pxl_read_source(name: String) -> Result<String, String> {
    let file = resolve_art_file(&name)?;
    std::fs::read_to_string(&file).map_err(|e| format!("Lecture impossible : {e}"))
}

/// Écrit (ou crée) une source d'art pxl. Retourne le chemin absolu écrit.
#[tauri::command]
fn pxl_write_source(name: String, content: String) -> Result<String, String> {
    let file = art_file_path(&name)?;
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Création du dossier : {e}"))?;
    }
    std::fs::write(&file, &content).map_err(|e| format!("Écriture impossible : {e}"))?;
    Ok(file.display().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Boot différé : la fenêtre s'ouvre immédiatement, le moteur s'initialise en
    // arrière-plan (scène, audio, glyphes). Les commandes IPC attendent la fin du
    // boot via `EngineState::lock` (timeout borné) sans bloquer le thread UI.
    let shared: SharedEngine = Arc::new(EngineState::new());
    shared.begin_boot();
    {
        let state = Arc::clone(&shared);
        std::thread::spawn(move || {
            let started = Instant::now();
            let result = NativeEngine::init();
            state.finish_boot(started, result);
        });
    }

    tauri::Builder::default()
        .manage(shared)
        .manage(PxlServe(Mutex::new(None)))
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            // Tue le serveur pxl à la fermeture de la fenêtre (pas de processus orphelin).
            let handle = app.handle().clone();
            if let Some(win) = app.get_webview_window("main") {
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        if let Some(mut child) = handle.state::<PxlServe>().0.lock().unwrap().take() {
                            let _ = child.kill();
                            let _ = child.wait();
                        }
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_engine_info,
            engine_status,
            engine_retry_boot,
            test_llm_connection,
            generate_ai_level,
            engine_frame,
            engine_frame_packet,
            engine_list_mesh_assets,
            engine_spawn_3d_preset,
            engine_new_3d_scene,
            engine_move_entity_3d,
            engine_canvas_width,
            engine_canvas_height,
            engine_on_key_down,
            engine_on_key_up,
            engine_on_mouse_move,
            engine_on_mouse_down,
            engine_on_mouse_up,
            engine_get_score,
            engine_get_fps,
            engine_get_cpu_us,
            engine_get_status_message,
            engine_telemetry,
            engine_toggle_radar,
            engine_toggle_spatial_grid,
            engine_toggle_editor_mode,
            engine_set_embedded_mode,
            engine_validate_recipe,
            engine_apply_recipe,
            engine_launch_archetype,
            engine_launch_archetype_3d,
            engine_apply_ai_payload,
            engine_accept_ai_patch,
            engine_reject_ai_patch,
            engine_has_pending_ai_patch,
            engine_set_active_tool,
            engine_get_active_tool,
            engine_cycle_brush_tile,
            engine_get_active_brush_tile,
            engine_export_standalone_html,
            engine_move_player_to,
            engine_get_player_pos,
            engine_interact,
            engine_new_blank_project,
            engine_load_project_json,
            engine_export_project_json,
            engine_get_entities_summary,
            engine_update_entity_transform,
            engine_delete_entity,
            engine_spawn_entity_preset,
            engine_editor_state,
            engine_select_entity,
            engine_get_selected_entity,
            engine_delete_selected_entity,
            engine_duplicate_selected_entity,
            engine_undo,
            engine_redo,
            engine_set_editor_mode,
            engine_set_brush_tile,
            engine_get_brush_tiles,
            engine_spawn_entity_at_screen,
            engine_save_scene,
            engine_list_sprite_assets,
            engine_sprite_png,
            engine_list_anim_assets,
            engine_list_sheet_assets,
            engine_sheet_png,
            engine_import_raster,
            engine_import_svg,
            engine_vectorize_raster,
            engine_list_vector_assets,
            engine_vector_load,
            engine_vector_save,
            engine_vector_spawn,
            pxl_available,
            pxl_build,
            pxl_serve_start,
            pxl_serve_stop,
            pxl_serve_status,
            pxl_list_sources,
            pxl_read_source,
            pxl_write_source,
            engine_get_kvl_graphs,
            engine_get_kvl_graph,
            engine_validate_kvl_graph,
            engine_save_kvl_graph,
            engine_build_guided_graph,
            engine_attach_script,
            engine_list_projects,
            engine_create_project,
            engine_open_project,
            engine_recent_projects,
            engine_remove_recent,
            engine_duplicate_project,
            engine_rename_project,
            engine_archive_project,
            engine_reveal_project,
            engine_system_diagnostics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
