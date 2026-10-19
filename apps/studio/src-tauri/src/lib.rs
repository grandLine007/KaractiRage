use serde::{Deserialize, Serialize};
use karactirage_wasm::ai_client::{AiClient, AiClientConfig, AiProvider};

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
            generate_ai_level
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
