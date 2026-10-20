use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use karactirage_engine::ai_client::{AiClient, AiClientConfig, AiProvider};
use karactirage_engine::native_api::NativeEngine;

type SharedEngine = Arc<Mutex<NativeEngine>>;

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
    engine: tauri::State<'_, SharedEngine>,
) -> bool {
    engine.lock().unwrap().update_entity_transform(id, x, y, w, h)
}

#[tauri::command]
fn engine_delete_entity(id: usize, engine: tauri::State<'_, SharedEngine>) -> bool {
    engine.lock().unwrap().delete_entity(id)
}

#[tauri::command]
fn engine_spawn_entity_preset(preset: String, x: f32, y: f32, engine: tauri::State<'_, SharedEngine>) -> usize {
    engine.lock().unwrap().spawn_entity_preset(&preset, x, y)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let engine = match NativeEngine::init() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[KaractiRage Studio] Échec d'initialisation du moteur : {}", e);
            std::process::exit(1);
        }
    };
    let shared: SharedEngine = Arc::new(Mutex::new(engine));

    tauri::Builder::default()
        .manage(shared)
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_engine_info,
            test_llm_connection,
            generate_ai_level,
            engine_frame,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
