use serde::{Deserialize, Serialize};
use std::process::Command;

/// Fournisseurs d'Intelligence Artificielle pris en charge par KaractiRage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiProvider {
    /// Modèle local hébergé via Ollama (ex: qwen2.5-coder, llama3, mistral)
    Ollama,
    /// Modèle local hébergé via LM Studio (compatible API OpenAI locale)
    LmStudio,
    /// Claude 3.5 Sonnet / Haiku par Anthropic
    Claude,
    /// Google Gemini 1.5 / 2.0 Flash (via endpoint OpenAI compatible)
    Gemini,
    /// DeepSeek Chat / Reasoner (haute performance raisonnement)
    DeepSeek,
    /// OpenAI GPT-4o / GPT-4o-mini
    OpenAI,
    /// Moteur local intégré KaractiRage (100% autonome, déterministe et sans réseau)
    OfflineLocal,
}

impl AiProvider {
    pub const ALL: [AiProvider; 7] = [
        AiProvider::Ollama,
        AiProvider::LmStudio,
        AiProvider::Claude,
        AiProvider::Gemini,
        AiProvider::DeepSeek,
        AiProvider::OpenAI,
        AiProvider::OfflineLocal,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            AiProvider::Ollama => "Ollama (Local)",
            AiProvider::LmStudio => "LM Studio (Local)",
            AiProvider::Claude => "Claude (Anthropic)",
            AiProvider::Gemini => "Gemini (Google)",
            AiProvider::DeepSeek => "DeepSeek",
            AiProvider::OpenAI => "OpenAI",
            AiProvider::OfflineLocal => "Atelier Local (Offline)",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            AiProvider::Ollama => "OLLAMA",
            AiProvider::LmStudio => "LM STUDIO",
            AiProvider::Claude => "CLAUDE",
            AiProvider::Gemini => "GEMINI",
            AiProvider::DeepSeek => "DEEPSEEK",
            AiProvider::OpenAI => "OPENAI",
            AiProvider::OfflineLocal => "OFFLINE",
        }
    }

    pub fn default_endpoint(&self) -> &'static str {
        match self {
            AiProvider::Ollama => "http://localhost:11434/v1/chat/completions",
            AiProvider::LmStudio => "http://localhost:1234/v1/chat/completions",
            AiProvider::Claude => "https://api.anthropic.com/v1/messages",
            AiProvider::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
            AiProvider::DeepSeek => "https://api.deepseek.com/v1/chat/completions",
            AiProvider::OpenAI => "https://api.openai.com/v1/chat/completions",
            AiProvider::OfflineLocal => "local://engine",
        }
    }

    pub fn default_model(&self) -> &'static str {
        match self {
            AiProvider::Ollama => "qwen2.5-coder",
            AiProvider::LmStudio => "local-model",
            AiProvider::Claude => "claude-3-5-sonnet-20241022",
            AiProvider::Gemini => "gemini-1.5-flash",
            AiProvider::DeepSeek => "deepseek-chat",
            AiProvider::OpenAI => "gpt-4o-mini",
            AiProvider::OfflineLocal => "karactirage-rule-engine-v1",
        }
    }

    pub fn env_var_name(&self) -> Option<&'static str> {
        match self {
            AiProvider::Claude => Some("ANTHROPIC_API_KEY"),
            AiProvider::Gemini => Some("GEMINI_API_KEY"),
            AiProvider::DeepSeek => Some("DEEPSEEK_API_KEY"),
            AiProvider::OpenAI => Some("OPENAI_API_KEY"),
            _ => None,
        }
    }

    pub fn get_api_key(&self) -> Option<String> {
        if let Some(var) = self.env_var_name() {
            std::env::var(var).ok().filter(|s| !s.trim().is_empty())
        } else {
            None
        }
    }

    pub fn is_ready(&self) -> bool {
        match self {
            AiProvider::OfflineLocal | AiProvider::Ollama | AiProvider::LmStudio => true,
            _ => self.get_api_key().is_some(),
        }
    }
}

/// Configuration active pour les requêtes IA
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiClientConfig {
    pub provider: AiProvider,
    pub custom_endpoint: Option<String>,
    pub custom_model: Option<String>,
    pub custom_api_key: Option<String>,
    pub timeout_seconds: u64,
}

impl AiClientConfig {
    /// Charge la configuration depuis karactirage.ai.json si présent, sinon utilise la configuration par défaut
    pub fn load_or_default() -> Self {
        if let Ok(content) = std::fs::read_to_string("karactirage.ai.json") {
            if let Ok(cfg) = serde_json::from_str::<AiClientConfig>(&content) {
                return cfg;
            }
        }
        Self::default()
    }

    /// Sauvegarde la configuration active dans un fichier JSON
    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).unwrap_or_default();
        std::fs::write(path, json)
    }
}

impl Default for AiClientConfig {
    fn default() -> Self {
        Self {
            provider: AiProvider::OfflineLocal,
            custom_endpoint: None,
            custom_model: None,
            custom_api_key: None,
            timeout_seconds: 25,
        }
    }
}

/// Payload JSON de réponse standard attendu de l'IA
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AiGeneratedGamePayload {
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub primary_genre: Option<String>,
    #[serde(default)]
    pub terrain: Option<AiGeneratedTerrain>,
    #[serde(default)]
    pub gameplay_loop: Option<AiGeneratedGameplayLoop>,
    #[serde(default)]
    pub entities: Vec<AiGeneratedEntity>,
    #[serde(default)]
    pub kvl_actions: Vec<AiGeneratedKvlAction>,
    #[serde(default)]
    pub variables: Option<std::collections::HashMap<String, String>>,
}

/// Structure du terrain et level design généré par l'IA
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AiGeneratedTerrain {
    #[serde(default = "default_theme")]
    pub theme: String, // "dungeon", "cyber", "forest", "lava_cave", "castle"
    #[serde(default)]
    pub platforms: Vec<AiGeneratedPlatform>,
    #[serde(default)]
    pub hazards: Vec<AiGeneratedHazard>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AiGeneratedPlatform {
    #[serde(default = "default_platform_name")]
    pub name: String,
    #[serde(alias = "x", default = "default_zero")]
    pub offset_x: f32,
    #[serde(alias = "y", default = "default_zero")]
    pub offset_y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default = "default_true")]
    pub is_solid: bool,
    #[serde(default)]
    pub color_hex: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AiGeneratedHazard {
    #[serde(default = "default_hazard_name")]
    pub name: String,
    #[serde(alias = "x", default = "default_zero")]
    pub offset_x: f32,
    #[serde(alias = "y", default = "default_zero")]
    pub offset_y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default = "default_hazard_damage")]
    pub damage: f32,
    #[serde(alias = "kind", default)]
    pub hazard_type: Option<String>, // "spikes", "lava", "poison"
}

/// Boucle de gameplay complète (Objectif, Condition de Victoire, Défaite)
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AiGeneratedGameplayLoop {
    #[serde(default = "default_objective")]
    pub objective: String,
    #[serde(default = "default_win_condition")]
    pub win_condition: String, // "CollectAllKeysThenExit", "DefeatBoss", "ReachGoal"
    #[serde(default = "default_lose_condition")]
    pub lose_condition: String, // "PlayerHealthZero"
    #[serde(default)]
    pub target_item_name: Option<String>,
    #[serde(default)]
    pub win_fanfare: Option<String>,
}

fn default_platform_name() -> String { "Plateforme".to_string() }
fn default_hazard_name() -> String { "Piege_Danger".to_string() }
fn default_theme() -> String { "dungeon".to_string() }
fn default_true() -> bool { true }
fn default_hazard_damage() -> f32 { 35.0 }
fn default_objective() -> String { "Explorer et triompher de l'épreuve".to_string() }
fn default_win_condition() -> String { "CollectAllKeysThenExit".to_string() }
fn default_lose_condition() -> String { "PlayerHealthZero".to_string() }

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AiGeneratedEntity {
    pub name: String,
    pub kind: String, // "player", "enemy", "boss", "item", "door", "hazard", "npc"
    #[serde(alias = "x", default = "default_zero")]
    pub offset_x: f32,
    #[serde(alias = "y", default = "default_zero")]
    pub offset_y: f32,
    #[serde(default = "default_size")]
    pub width: f32,
    #[serde(default = "default_size")]
    pub height: f32,
    #[serde(default = "default_health")]
    pub health: f32,
    #[serde(default)]
    pub color_hex: Option<String>,
    #[serde(default)]
    pub sprite_archetype: Option<String>, // "ninja", "knight", "drone", "slime", "skull", "chest", "key", "potion", "portal", "spikes", "coin"
    #[serde(default)]
    pub sprite_ascii: Option<Vec<String>>, // Matrice personnalisée 16x16 de caractères
    #[serde(default)]
    pub sprite_palette: Option<std::collections::HashMap<String, String>>, // Caractère -> Couleur Hex
    #[serde(default)]
    pub sprite_asset: Option<String>, // Asset pxl prégénéré (assets/generated/sprites/<asset>.png)
    #[serde(default)]
    pub sprite_frame: Option<String>, // Frame nommée d'une spritesheet (avec sprite_asset)
    #[serde(default)]
    pub movement: Option<String>, // "patrol", "chase", "jump", "none"
    #[serde(default)]
    pub has_collider: bool,
    #[serde(default)]
    pub has_vision: bool,
    #[serde(default)]
    pub is_flocking: bool,
    // --- Champs 3D (Lot 7) ---
    /// Asset de mesh 3D référencé dans le manifest (pillar, rock, platform, …).
    #[serde(default)]
    pub mesh_asset: Option<String>,
    /// Primitive 3D directe : "cube", "sphere", "plane", "capsule".
    #[serde(default)]
    pub mesh_primitive: Option<String>,
    /// Position 3D [x, y, z] (sinon dérivée de offset_x/offset_y).
    #[serde(default)]
    pub position3d: Option<[f32; 3]>,
    /// Rotation d'Euler en degrés [x, y, z].
    #[serde(default)]
    pub rotation_euler_deg: Option<[f32; 3]>,
    /// Échelle [sx, sy, sz].
    #[serde(default)]
    pub scale3d: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AiGeneratedKvlAction {
    pub event: String,
    pub action: String,
    #[serde(default)]
    pub target_entity: Option<String>,
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub on_true_action: Option<String>,
    #[serde(default)]
    pub on_false_action: Option<String>,
    #[serde(default)]
    pub second_action: Option<String>,
}

fn default_zero() -> f32 { 0.0 }
fn default_size() -> f32 { 32.0 }
fn default_health() -> f32 { 3.0 }

/// Spécification de référence universelle KaractiRage intégrée à la compilation
pub const KARACTI_AI_SPECIFICATION_EMBEDDED: &str = include_str!("../../../KARACTI_AI_SPEC.md");

/// Récupère le prompt système et la spécification de référence RAG pour les LLMs (Version complète)
pub fn get_karactirage_system_prompt() -> String {
    // Si un fichier KARACTI_AI_SPEC.md existe dans le système de fichiers, lecture dynamique à chaud
    if let Ok(spec) = std::fs::read_to_string("KARACTI_AI_SPEC.md") {
        return format_system_prompt_with_spec(&spec);
    }
    if let Ok(spec) = std::fs::read_to_string("../../KARACTI_AI_SPEC.md") {
        return format_system_prompt_with_spec(&spec);
    }
    format_system_prompt_with_spec(KARACTI_AI_SPECIFICATION_EMBEDDED)
}

/// Prompt système haute densité et ultra-rapide optimisé pour les modèles locaux (LM Studio, Ollama)
pub fn get_karactirage_compact_system_prompt() -> String {
    r##"You are the Lead Game Architect & Technical Artist for KaractiRage 2D engine.
CRITICAL: Keep internal thinking under 30 words. Output ONLY raw JSON (no markdown, no ```json codeblocks) conforming strictly to this schema:
{
  "title": "Level Title",
  "description": "Short narrative description",
  "primary_genre": "Platformer",
  "terrain": {
    "platforms": [{"x": -200, "y": -100, "width": 400, "height": 24, "color_hex": "#334155"}],
    "hazards": [{"x": 0, "y": -150, "width": 60, "height": 20, "kind": "lava", "color_hex": "#EF4444"}]
  },
  "gameplay_loop": {
    "objective": "Mission objective text",
    "win_condition": "DefeatBoss",
    "loss_condition": "PlayerDeath"
  },
  "entities": [
    {
      "name": "Ninja_Hero",
      "kind": "player",
      "offset_x": -120.0,
      "offset_y": 0.0,
      "width": 32.0,
      "height": 32.0,
      "health": 5.0,
      "color_hex": "#3B82F6",
      "sprite_archetype": "ninja",
      "movement": "jump",
      "has_collider": true,
      "has_vision": true,
      "is_flocking": false
    },
    {
      "name": "Boss_Demon",
      "kind": "boss",
      "offset_x": 160.0,
      "offset_y": 0.0,
      "width": 48.0,
      "height": 48.0,
      "health": 10.0,
      "color_hex": "#DC2626",
      "sprite_archetype": "boss",
      "movement": "chase",
      "has_collider": true,
      "has_vision": true,
      "is_flocking": false
    }
  ],
  "kvl_actions": [
    {
      "event": "OnEntityDefeated",
      "action": "WinGame",
      "target_entity": "Boss_Demon"
    }
  ]
}

Available sprite_archetype values: "ninja", "knight", "drone", "slime", "skull", "boss", "chest", "key", "potion", "portal", "coin", "spikes".
Available sprite_asset values (pixel-art pxl prégénéré): "coin", "potion", "key", "chest", "door", "platform", "spikes", "crate", "plate", "boss", "wall". Use "sprite_frame" (ex: "coin") with "sprite_asset":"chars" to pick a frame from the atlas.
Available movement values: "patrol", "chase", "jump", "none".
Available win_condition values: "DefeatBoss", "CollectAllKeysThenExit", "SurviveTimer".
Available loss_condition values: "PlayerDeath", "TimeOut".
Output raw JSON only."##.to_string()
}

/// Sélectionne le prompt système le plus adapté selon le fournisseur (taille de contexte et vitesse locale vs cloud)
pub fn get_system_prompt_for_provider(provider: AiProvider) -> String {
    match provider {
        AiProvider::LmStudio | AiProvider::Ollama => get_karactirage_compact_system_prompt(),
        _ => get_karactirage_system_prompt(),
    }
}

fn format_system_prompt_with_spec(spec: &str) -> String {
    format!(
        "You are the Lead Game Architect & Technical Artist for KaractiRage, a high-performance 2D engine.\n\
        Your mission is to generate a COMPLETE, COHESIVE, PLAYABLE 2D game level matching the user's creative request.\n\
        You must ground your design strictly on the official KaractiRage engine specification below.\n\
        You must provide terrain (platforms, walls, hazards), pixel-art sprites or archetypes (ninja, knight, drone, slime, skull, boss, chest, key, potion, portal, coin, spikes), animated entities, and a complete gameplay loop with clear win and loss conditions.\n\n\
        You MUST reply with ONLY a raw JSON object (no markdown formatting, no ```json codeblocks, no conversational text) matching the schema described in the specification.\n\n\
        === OFFICIAL KARACTIRAGE ENGINE SPECIFICATION (GROUND TRUTH / RAG) ===\n\
        {}\n\
        === END OF SPECIFICATION ===\n",
        spec
    )
}

/// Pont d'exécution HTTP via curl natif (compatible macOS, Linux, Windows 10/11)
pub struct AiClient;

impl AiClient {
    /// Effectue un appel réseau vers le fournisseur configuré
    pub fn send_prompt(config: &AiClientConfig, prompt: &str) -> Result<AiGeneratedGamePayload, String> {
        if config.provider == AiProvider::OfflineLocal {
            return Err("Fournisseur OfflineLocal : utilisez AiPatchBuilder localement.".into());
        }

        let endpoint = config.custom_endpoint.as_deref().unwrap_or_else(|| config.provider.default_endpoint());
        let model = config.custom_model.as_deref().unwrap_or_else(|| config.provider.default_model());
        let api_key = config.custom_api_key.clone().or_else(|| config.provider.get_api_key());

        let system_prompt = get_system_prompt_for_provider(config.provider);

        let raw_output = if config.provider == AiProvider::Claude {
            Self::call_claude(endpoint, model, api_key.as_deref(), &system_prompt, prompt, config.timeout_seconds)?
        } else {
            Self::call_openai_compatible(endpoint, model, api_key.as_deref(), &system_prompt, prompt, config.timeout_seconds)?
        };

        Self::parse_llm_json_response(&raw_output)
    }

    /// Appel HTTP standardisé pour API OpenAI-compatible (Ollama, LM Studio, Gemini, DeepSeek, OpenAI)
    fn call_openai_compatible(
        endpoint: &str,
        model: &str,
        api_key: Option<&str>,
        system_prompt: &str,
        user_prompt: &str,
        timeout_secs: u64,
    ) -> Result<String, String> {
        let body = serde_json::json!({
            "model": model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt }
            ],
            "max_tokens": 4096,
            "temperature": 0.2
        });

        let body_str = body.to_string();

        let mut cmd = Command::new("curl");
        cmd.arg("-s")
           .arg("-X").arg("POST")
           .arg("-H").arg("Content-Type: application/json");

        if let Some(key) = api_key {
            cmd.arg("-H").arg(format!("Authorization: Bearer {}", key));
        }

        cmd.arg("-d").arg(&body_str)
           .arg("--max-time").arg(timeout_secs.to_string())
           .arg(endpoint);

        let output = cmd.output().map_err(|e| format!("Échec d'exécution de curl : {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Erreur curl ({}) : {}", output.status, stderr));
        }

        let response_str = String::from_utf8_lossy(&output.stdout).to_string();

        if response_str.trim().is_empty() {
            return Err("Réponse vide reçue du modèle LLM.".into());
        }

        let resp_json: serde_json::Value = serde_json::from_str(&response_str)
            .map_err(|e| format!("Réponse non-JSON du serveur : {} (Brut: {})", e, response_str))?;

        if let Some(err) = resp_json.get("error") {
            return Err(format!("Erreur retournée par le LLM : {}", err));
        }

        let msg = &resp_json["choices"][0]["message"];
        let content = if let Some(c) = msg["content"].as_str().filter(|s| !s.trim().is_empty()) {
            c.to_string()
        } else if let Some(r) = msg["reasoning_content"].as_str().filter(|s| s.contains('{') && s.contains('}')) {
            // Dans certains reasoning models tronqués, le JSON a pu être produit dans la chaîne de pensée
            r.to_string()
        } else {
            return Err(format!("Format inattendu choices[0].message.content : {}", response_str));
        };

        Ok(content)
    }

    /// Appel HTTP spécialisé pour l'API Anthropic Claude (/v1/messages)
    fn call_claude(
        endpoint: &str,
        model: &str,
        api_key: Option<&str>,
        system_prompt: &str,
        user_prompt: &str,
        timeout_secs: u64,
    ) -> Result<String, String> {
        let key = api_key.ok_or_else(|| "Clé API Claude manquante (définissez ANTHROPIC_API_KEY).".to_string())?;

        let body = serde_json::json!({
            "model": model,
            "system": system_prompt,
            "messages": [
                { "role": "user", "content": user_prompt }
            ],
            "max_tokens": 4096,
            "temperature": 0.2
        });

        let body_str = body.to_string();

        let mut cmd = Command::new("curl");
        cmd.arg("-s")
           .arg("-X").arg("POST")
           .arg("-H").arg("Content-Type: application/json")
           .arg("-H").arg(format!("x-api-key: {}", key))
           .arg("-H").arg("anthropic-version: 2023-06-01")
           .arg("-d").arg(&body_str)
           .arg("--max-time").arg(timeout_secs.to_string())
           .arg(endpoint);

        let output = cmd.output().map_err(|e| format!("Échec d'exécution de curl : {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Erreur curl ({}) : {}", output.status, stderr));
        }

        let response_str = String::from_utf8_lossy(&output.stdout).to_string();
        let resp_json: serde_json::Value = serde_json::from_str(&response_str)
            .map_err(|e| format!("Réponse non-JSON de Claude : {} (Brut: {})", e, response_str))?;

        if let Some(err) = resp_json.get("error") {
            return Err(format!("Erreur Claude : {}", err));
        }

        let content = resp_json["content"][0]["text"]
            .as_str()
            .ok_or_else(|| format!("Format inattendu content[0].text : {}", response_str))?;

        Ok(content.to_string())
    }

    /// Nettoie les balises markdown (```json ... ```) et extrait le JSON valide
    pub fn parse_llm_json_response(raw: &str) -> Result<AiGeneratedGamePayload, String> {
        let trimmed = raw.trim();
        let json_str = if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                &trimmed[start..=end]
            } else {
                trimmed
            }
        } else {
            trimmed
        };

        serde_json::from_str::<AiGeneratedGamePayload>(json_str)
            .map_err(|e| format!("Échec de parsing JSON du modèle : {} (Extrait: '{}')", e, json_str))
    }

    /// Vérifie la connectivité réelle du fournisseur sélectionné (ping curl rapide ou vérification de clé)
    pub fn test_connection(config: &AiClientConfig) -> Result<String, String> {
        match config.provider {
            AiProvider::OfflineLocal => {
                Ok("Moteur Local Déterministe 100% autonome et hors-ligne (Opérationnel)".to_string())
            }
            AiProvider::Ollama => {
                let endpoint = config.custom_endpoint.as_deref().unwrap_or("http://localhost:11434");
                let base_url = endpoint.split("/api/").next().unwrap_or(endpoint);
                let test_url = format!("{}/api/tags", base_url.trim_end_matches('/'));
                let mut cmd = Command::new("curl");
                cmd.arg("-s")
                   .arg("-m").arg("2")
                   .arg(&test_url);
                match cmd.output() {
                    Ok(output) if output.status.success() => {
                        let text = String::from_utf8_lossy(&output.stdout);
                        if text.contains("models") {
                            Ok(format!("Ollama connecté ({}) • Serveur actif !", base_url))
                        } else {
                            Ok(format!("Ollama répond sur {}", base_url))
                        }
                    }
                    _ => Err(format!("Ollama injoignable sur {} • Lancez 'ollama serve'", base_url)),
                }
            }
            AiProvider::LmStudio => {
                let endpoint = config.custom_endpoint.as_deref().unwrap_or("http://localhost:1234");
                let base_url = endpoint.split("/v1/").next().unwrap_or(endpoint);
                let test_url = format!("{}/v1/models", base_url.trim_end_matches('/'));
                let mut cmd = Command::new("curl");
                cmd.arg("-s")
                   .arg("-m").arg("2")
                   .arg(&test_url);
                match cmd.output() {
                    Ok(output) if output.status.success() => {
                        Ok(format!("LM Studio connecté ({}) • Serveur local prêt !", base_url))
                    }
                    _ => Err(format!("LM Studio injoignable sur {} • Démarrez le serveur local dans LM Studio", base_url)),
                }
            }
            AiProvider::Claude => {
                let key = config.custom_api_key.clone().or_else(|| config.provider.get_api_key());
                if let Some(k) = key {
                    let preview = if k.len() > 8 { format!("{}...", &k[..8]) } else { "***".to_string() };
                    Ok(format!("Clé ANTHROPIC_API_KEY détectée ({}) • Prêt", preview))
                } else {
                    Err("Clé absente • Définissez 'export ANTHROPIC_API_KEY=\"sk-ant-...\"'".to_string())
                }
            }
            AiProvider::Gemini => {
                let key = config.custom_api_key.clone().or_else(|| config.provider.get_api_key());
                if let Some(k) = key {
                    let preview = if k.len() > 8 { format!("{}...", &k[..8]) } else { "***".to_string() };
                    Ok(format!("Clé GEMINI_API_KEY détectée ({}) • Prêt", preview))
                } else {
                    Err("Clé absente • Définissez 'export GEMINI_API_KEY=\"AIza...\"'".to_string())
                }
            }
            AiProvider::DeepSeek => {
                let key = config.custom_api_key.clone().or_else(|| config.provider.get_api_key());
                if let Some(k) = key {
                    let preview = if k.len() > 8 { format!("{}...", &k[..8]) } else { "***".to_string() };
                    Ok(format!("Clé DEEPSEEK_API_KEY détectée ({}) • Prêt", preview))
                } else {
                    Err("Clé absente • Définissez 'export DEEPSEEK_API_KEY=\"sk-...\"'".to_string())
                }
            }
            AiProvider::OpenAI => {
                let key = config.custom_api_key.clone().or_else(|| config.provider.get_api_key());
                if let Some(k) = key {
                    let preview = if k.len() > 8 { format!("{}...", &k[..8]) } else { "***".to_string() };
                    Ok(format!("Clé OPENAI_API_KEY détectée ({}) • Prêt", preview))
                } else {
                    Err("Clé absente • Définissez 'export OPENAI_API_KEY=\"sk-...\"'".to_string())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_provider_metadata_and_defaults() {
        for provider in AiProvider::ALL {
            assert!(!provider.label().is_empty());
            assert!(!provider.short_label().is_empty());
            assert!(!provider.default_endpoint().is_empty());
            assert!(!provider.default_model().is_empty());
        }
    }

    #[test]
    fn test_parse_llm_json_response_with_markdown_fences() {
        let raw = r##"
Here is the requested game architecture:
```json
{
  "title": "Ninja Cyber Platformer",
  "description": "Un robot ninja affrontant des drones de patrouille.",
  "primary_genre": "Platformer",
  "entities": [
    {
      "name": "Drone_Patrouilleur",
      "kind": "enemy",
      "offset_x": 120.0,
      "offset_y": -30.0,
      "width": 28.0,
      "height": 28.0,
      "health": 3.0,
      "color_hex": "#EF4444",
      "movement": "patrol",
      "has_collider": true,
      "has_vision": true,
      "is_flocking": false
    }
  ],
  "kvl_actions": [
    {
      "event": "OnVisionAlert",
      "action": "NavChasePlayer:130.0"
    }
  ]
}
```
Good luck!
"##;
        let parsed = AiClient::parse_llm_json_response(raw).expect("Parsing payload");
        assert_eq!(parsed.title, "Ninja Cyber Platformer");
        assert_eq!(parsed.entities.len(), 1);
        assert_eq!(parsed.entities[0].name, "Drone_Patrouilleur");
        assert_eq!(parsed.kvl_actions.len(), 1);
        assert_eq!(parsed.kvl_actions[0].event, "OnVisionAlert");
    }

    #[test]
    fn test_parse_full_level_payload_with_terrain_pixel_sprites_and_gameplay_loop() {
        let raw = r##"{
  "title": "Crypte du Golem Solaire",
  "description": "Niveau de plateforme-aventure avec sol dallé, gouffre de lave et golem gardien.",
  "primary_genre": "Platformer",
  "terrain": {
    "theme": "dungeon",
    "platforms": [
      {
        "name": "Ground_Floor",
        "offset_x": -200.0,
        "offset_y": 150.0,
        "width": 600.0,
        "height": 40.0,
        "is_solid": true,
        "color_hex": "#1E293B"
      },
      {
        "name": "High_Ledge",
        "offset_x": 100.0,
        "offset_y": 50.0,
        "width": 140.0,
        "height": 20.0,
        "is_solid": true,
        "color_hex": "#334155"
      }
    ],
    "hazards": [
      {
        "name": "Spike_Trap",
        "offset_x": 20.0,
        "offset_y": 130.0,
        "width": 60.0,
        "height": 20.0,
        "damage": 40.0,
        "hazard_type": "spikes"
      }
    ]
  },
  "gameplay_loop": {
    "objective": "Collecter la Clé Solaire et ouvrir la Porte des Étoiles",
    "win_condition": "CollectAllKeysThenExit",
    "lose_condition": "PlayerHealthZero",
    "target_item_name": "Cle_Solaire",
    "win_fanfare": "victory_bell"
  },
  "entities": [
    {
      "name": "Ninja_Hero",
      "kind": "player",
      "offset_x": 0.0,
      "offset_y": 100.0,
      "width": 32.0,
      "height": 32.0,
      "health": 6.0,
      "color_hex": "#38BDF8",
      "sprite_archetype": "ninja",
      "has_collider": true,
      "has_vision": false,
      "is_flocking": false
    },
    {
      "name": "Golem_Gardien",
      "kind": "boss",
      "offset_x": 250.0,
      "offset_y": 80.0,
      "width": 48.0,
      "height": 48.0,
      "health": 12.0,
      "color_hex": "#F59E0B",
      "sprite_archetype": "boss",
      "has_collider": true,
      "has_vision": true,
      "is_flocking": false
    },
    {
      "name": "Cle_Solaire",
      "kind": "item",
      "offset_x": 120.0,
      "offset_y": 20.0,
      "width": 24.0,
      "height": 24.0,
      "health": 0.0,
      "color_hex": "#FBBF24",
      "sprite_archetype": "key",
      "has_collider": true,
      "has_vision": false,
      "is_flocking": false
    }
  ],
  "kvl_actions": [
    {
      "event": "OnStart",
      "action": "SetObjective:Collecter la Clé Solaire et ouvrir la Porte des Étoiles"
    }
  ]
}"##;
        let parsed = AiClient::parse_llm_json_response(raw).expect("Parsing full level");
        assert_eq!(parsed.title, "Crypte du Golem Solaire");
        assert!(parsed.terrain.is_some());
        let terrain = parsed.terrain.unwrap();
        assert_eq!(terrain.platforms.len(), 2);
        assert_eq!(terrain.hazards.len(), 1);
        assert!(parsed.gameplay_loop.is_some());
        let loop_spec = parsed.gameplay_loop.unwrap();
        assert_eq!(loop_spec.win_condition, "CollectAllKeysThenExit");
        assert_eq!(parsed.entities.len(), 3);
        assert_eq!(parsed.entities[0].sprite_archetype.as_deref(), Some("ninja"));
        assert_eq!(parsed.entities[1].sprite_archetype.as_deref(), Some("boss"));
    }

    #[test]
    fn test_karactirage_system_prompt_contains_rag_specification() {
        let prompt = get_karactirage_system_prompt();
        assert!(!prompt.is_empty());
        assert!(prompt.contains("OFFICIAL KARACTIRAGE ENGINE SPECIFICATION"));
        assert!(prompt.contains("ShapeKind::PixelMatrix"));
        assert!(prompt.contains("ninja"));
        assert!(prompt.contains("skull"));
        assert!(prompt.contains("platforms"));
        assert!(prompt.contains("hazards"));
        assert!(prompt.contains("gameplay_loop"));
        assert!(prompt.contains("kvl_actions"));
        assert!(KARACTI_AI_SPECIFICATION_EMBEDDED.len() > 1000);
    }

    #[test]
    fn test_system_prompt_provider_selection() {
        let lm_prompt = get_system_prompt_for_provider(AiProvider::LmStudio);
        assert!(lm_prompt.contains("Keep internal thinking under 30 words"));
        assert!(lm_prompt.contains("platforms"));
        assert!(lm_prompt.contains("ninja"));
        assert!(lm_prompt.len() < 2500); // Format dense et compact

        let claude_prompt = get_system_prompt_for_provider(AiProvider::Claude);
        assert!(claude_prompt.contains("OFFICIAL KARACTIRAGE ENGINE SPECIFICATION"));
    }

    #[test]
    #[ignore]
    fn test_live_lm_studio_nemotron_end_to_end() {
        let config = AiClientConfig {
            provider: AiProvider::LmStudio,
            custom_endpoint: Some("http://127.0.0.1:1234/v1/chat/completions".to_string()),
            custom_model: Some("nvidia/nemotron-3-nano-4b".to_string()),
            custom_api_key: None,
            timeout_seconds: 120,
        };

        let conn = AiClient::test_connection(&config).expect("LM Studio connection");
        assert!(conn.contains("LM Studio connecté"));

        let payload = AiClient::send_prompt(&config, "Crée un donjon sombre avec un héros ninja, un piège de lave et un boss démon")
            .expect("Live LM Studio generation");
        println!("[LM STUDIO TEST] Title: {}", payload.title);
        println!("[LM STUDIO TEST] Entities count: {}", payload.entities.len());
        println!("[LM STUDIO TEST] Description: {}", payload.description);

        assert!(!payload.title.is_empty());
        assert!(!payload.entities.is_empty());

        let patch = crate::ai_patch::AiPatchBuilder::build_from_generated_payload(
            &payload,
            "Crée un donjon sombre",
            (0.0, 0.0),
            100,
            &[],
            None,
        );

        println!("[LM STUDIO TEST] Sandbox static passed: {}", patch.sandbox_report.static_check_passed);
        println!("[LM STUDIO TEST] Sandbox static errors: {}", patch.sandbox_report.static_errors_count);
        println!("[LM STUDIO TEST] Sandbox simulation passed: {}", patch.sandbox_report.simulation_passed);

        assert!(patch.sandbox_report.static_check_passed);
        assert_eq!(patch.sandbox_report.static_errors_count, 0);
        assert!(patch.sandbox_report.simulation_passed);
    }
}

/// Tests de contrat partagés avec le sidecar Python `agents/` (cf. PLAN_CONCEPTION_CREWAI.md, jalon M0).
/// Le JSON Schema est émis ici (source de vérité Rust) puis validé côté Python via jsonschema/Pydantic
/// sur les MÊMES fixtures — toute divergence de schéma casse les deux CIs.
#[cfg(test)]
mod contract_tests {
    use super::*;

    /// Contrat versionné partagé (émis par ce test, consommé par `agents/tests/test_contract.py`).
    const SCHEMA_PATH: &str = "../../agents/contracts/level.schema.json";
    /// Fixtures JSON partagées entre Rust (désérialisation + sandbox) et Python (jsonschema).
    const FIXTURES_DIR: &str = "../../agents/fixtures";

    fn generated_schema_value() -> serde_json::Value {
        let schema = schemars::schema_for!(AiGeneratedGamePayload);
        serde_json::to_value(&schema).expect("Sérialisation du JSON Schema")
    }

    /// Vérifie que `agents/contracts/level.schema.json` est identique au schéma émis par les structs Rust.
    /// Régénération : `UPDATE_CONTRACTS=1 cargo test -p engine-core contract_schema_is_current`
    #[test]
    fn contract_schema_is_current() {
        let generated = generated_schema_value();

        if std::env::var("UPDATE_CONTRACTS").is_ok() {
            let dir = std::path::Path::new(SCHEMA_PATH).parent().expect("parent");
            std::fs::create_dir_all(dir).expect("création agents/contracts");
            std::fs::write(
                SCHEMA_PATH,
                serde_json::to_string_pretty(&generated).expect("pretty") + "\n",
            )
            .expect("écriture du contrat");
        }

        let raw = std::fs::read_to_string(SCHEMA_PATH).unwrap_or_else(|e| {
            panic!(
                "Contrat absent : {SCHEMA_PATH} ({e}) — régénérez avec UPDATE_CONTRACTS=1 cargo test -p engine-core contract_schema_is_current"
            )
        });
        let on_disk: serde_json::Value = serde_json::from_str(&raw).expect("contrat JSON valide");
        assert_eq!(
            on_disk, generated,
            "Contrat désynchronisé des structs Rust — régénérez avec UPDATE_CONTRACTS=1 cargo test -p engine-core contract_schema_is_current"
        );
    }

    /// Les fixtures partagées doivent (1) se désérialiser dans les structs moteur et
    /// (2) survivre aux 4 portes de validation (sandbox statique + simulation 100 pas, spec §8).
    #[test]
    fn contract_fixtures_pass_engine_and_sandbox() {
        let mut checked = 0usize;
        let mut paths: Vec<_> = std::fs::read_dir(FIXTURES_DIR)
            .unwrap_or_else(|e| panic!("Répertoire fixtures absent : {FIXTURES_DIR} ({e})"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .collect();
        paths.sort();

        for path in paths {
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Lecture {} : {e}", path.display()));
            let payload: AiGeneratedGamePayload = serde_json::from_str(&content)
                .unwrap_or_else(|e| panic!("Fixture {} invalide pour le moteur : {e}", path.display()));

            let patch = crate::ai_patch::AiPatchBuilder::build_from_generated_payload(
                &payload, "contract test", (100.0, 100.0), 100, &[], None,
            );
            assert!(
                patch.sandbox_report.static_check_passed,
                "Fixture {} : contrôle statique échoué",
                path.display()
            );
            assert_eq!(
                patch.sandbox_report.static_errors_count, 0,
                "Fixture {} : erreurs statiques",
                path.display()
            );
            assert!(
                patch.sandbox_report.simulation_passed,
                "Fixture {} : simulation 100 pas échouée (spéc §8)",
                path.display()
            );
            checked += 1;
        }

        assert!(checked >= 2, "Au moins 2 fixtures attendues dans {FIXTURES_DIR}");
        println!("[contrat] {checked} fixtures validées (moteur + sandbox)");
    }
}

