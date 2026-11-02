//! Client LLM **streamant** + parsing JSON partiel (SSE OpenAI-compatible).
//!
//! Complète `ai_client::AiClient` (bloquant) : `stream_prompt` envoie `"stream": true`
//! et consomme les événements SSE (`data: …`) en accumulant `choices[].delta.content`.
//! La complétude du JSON accumulé est suivie via `json_completeness`.

use serde_json::Value;

use crate::ai_client::{AiClientConfig, AiProvider, LlmBackend};

/// État de complétude d'un texte JSON accumulé au fil du flux.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonCompleteness {
    /// Aucune accolade ouvrante reçue (ou texte vide).
    Empty,
    /// Un objet JSON est en cours (accolades non équilibrées).
    Partial,
    /// Un objet JSON complet est présent (accolades équilibrées).
    Complete,
}

/// Analyse (heuristique) la complétude d'un JSON en accumulation, sans le parser
/// entièrement. Ignore le texte avant la première `{`, tient compte des chaînes.
pub fn json_completeness(text: &str) -> JsonCompleteness {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut saw_open = false;

    for c in text.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => {
                depth += 1;
                saw_open = true;
            }
            '}' => depth -= 1,
            _ => {}
        }
    }

    if !saw_open {
        JsonCompleteness::Empty
    } else if depth <= 0 && !in_string {
        JsonCompleteness::Complete
    } else {
        JsonCompleteness::Partial
    }
}

/// Parseur SSE incrémental : extrait `choices[].delta.content` de chunks arbitraires
/// (les lignes `data:` peuvent être coupées entre deux lectures).
#[derive(Debug, Default)]
pub struct SseParser {
    buffer: String,
    content: String,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Consomme un fragment de flux ; notifie `on_delta` pour chaque morceau de contenu.
    pub fn feed(&mut self, chunk: &str, on_delta: &mut dyn FnMut(&str)) {
        self.buffer.push_str(chunk);
        while let Some(pos) = self.buffer.find('\n') {
            let line: String = self.buffer.drain(..=pos).collect();
            let line = line.trim_end_matches(['\r', '\n']);
            if let Some(data) = line.strip_prefix("data:").map(str::trim) {
                if data.is_empty() || data == "[DONE]" {
                    continue;
                }
                if let Ok(value) = serde_json::from_str::<Value>(data) {
                    if let Some(delta) = value["choices"][0]["delta"]["content"].as_str() {
                        if !delta.is_empty() {
                            self.content.push_str(delta);
                            on_delta(delta);
                        }
                    }
                }
            }
        }
    }

    /// Contenu accumulé jusqu'ici.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Consomme le parseur et renvoie le contenu accumulé.
    pub fn into_content(self) -> String {
        self.content
    }
}

/// Appel streaming OpenAI-compatible (Ollama, LM Studio, Gemini, DeepSeek, OpenAI).
///
/// `on_delta` est notifié pour chaque fragment de contenu ; `cancelled` permet
/// d'interrompre la génération. Renvoie le contenu complet accumulé (le JSON payload),
/// que l'appelant confie à `AiClient::parse_llm_json_response`.
#[allow(clippy::too_many_arguments)]
pub fn stream_prompt(
    config: &AiClientConfig,
    system_prompt: &str,
    user_prompt: &str,
    backend: &dyn LlmBackend,
    on_delta: &mut dyn FnMut(&str),
    cancelled: &dyn Fn() -> bool,
) -> Result<String, String> {
    if config.provider == AiProvider::OfflineLocal {
        return Err("Fournisseur OfflineLocal : streaming non disponible.".into());
    }
    if config.provider == AiProvider::Claude {
        return Err("Streaming Claude non supporté (utilisez un fournisseur OpenAI-compatible).".into());
    }
    if cancelled() {
        return Err("Génération annulée.".into());
    }

    let endpoint = config
        .custom_endpoint
        .as_deref()
        .unwrap_or_else(|| config.provider.default_endpoint());
    let model = config
        .custom_model
        .as_deref()
        .unwrap_or_else(|| config.provider.default_model());
    let api_key = config
        .custom_api_key
        .clone()
        .or_else(|| config.provider.get_api_key());

    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt }
        ],
        "max_tokens": 4096,
        "temperature": 0.2,
        "stream": true
    });
    let body_str = body.to_string();

    let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
    if let Some(key) = api_key {
        headers.push(("Authorization".to_string(), format!("Bearer {key}")));
    }

    let mut parser = SseParser::new();
    let mut aborted = false;

    backend.post_json_stream(
        endpoint,
        &headers,
        &body_str,
        config.timeout_seconds,
        &mut |chunk| {
            if cancelled() {
                aborted = true;
                return;
            }
            parser.feed(chunk, &mut |delta| on_delta(delta));
        },
    )?;

    if aborted {
        return Err("Génération annulée.".into());
    }
    let content = parser.into_content();
    if content.trim().is_empty() {
        return Err("Aucun contenu reçu du flux LLM.".into());
    }
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_client::{AiClient, AiProvider};

    struct MockStreamBackend {
        chunks: Vec<String>,
    }

    impl LlmBackend for MockStreamBackend {
        fn post_json(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            _body: &str,
            _timeout_secs: u64,
        ) -> Result<String, String> {
            Ok(self.chunks.concat())
        }

        fn post_json_stream(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            _body: &str,
            _timeout_secs: u64,
            on_chunk: &mut dyn FnMut(&str),
        ) -> Result<String, String> {
            let mut acc = String::new();
            for c in &self.chunks {
                acc.push_str(c);
                on_chunk(c);
            }
            Ok(acc)
        }
    }

    fn config() -> AiClientConfig {
        AiClientConfig {
            provider: AiProvider::LmStudio,
            custom_endpoint: None,
            custom_model: None,
            custom_api_key: None,
            timeout_seconds: 10,
        }
    }

    #[test]
    fn json_completeness_tracks_brace_balance() {
        assert_eq!(json_completeness(""), JsonCompleteness::Empty);
        assert_eq!(json_completeness("préambule sans objet"), JsonCompleteness::Empty);
        assert_eq!(json_completeness("{\"a\":"), JsonCompleteness::Partial);
        // Les accolades dans les chaînes ne faussent pas le compte.
        assert_eq!(json_completeness("{\"a\":\"}\"}"), JsonCompleteness::Complete);
    }

    #[test]
    fn sse_parser_handles_split_lines_and_done() {
        let mut parser = SseParser::new();
        let mut deltas = Vec::new();

        // Ligne coupée entre deux chunks.
        parser.feed("data: {\"choices\":[{\"delta\":{\"content\":\"Bon\"}}", &mut |d| deltas.push(d.to_string()));
        parser.feed("]}\n", &mut |d| deltas.push(d.to_string()));
        parser.feed("data: {\"choices\":[{\"delta\":{\"content\":\"jour\"}}]}\n", &mut |d| deltas.push(d.to_string()));
        parser.feed("data: [DONE]\n", &mut |d| deltas.push(d.to_string()));

        assert_eq!(deltas, vec!["Bon".to_string(), "jour".to_string()]);
        assert_eq!(parser.content(), "Bonjour");
    }

    #[test]
    fn stream_prompt_extracts_payload_from_sse() {
        let payload = r#"{"title":"Mock Stream","description":"x","entities":[],"kvl_actions":[]}"#;
        // Fractionne le payload en plusieurs événements SSE (contenu JSON-échappé).
        let mid = payload.len() / 2;
        let part1 = serde_json::to_string(&payload[..mid]).unwrap();
        let part2 = serde_json::to_string(&payload[mid..]).unwrap();
        let chunks = vec![
            format!("data: {{\"choices\":[{{\"delta\":{{\"content\":{}}}}}]}}\n", part1),
            format!("data: {{\"choices\":[{{\"delta\":{{\"content\":{}}}}}]}}\n", part2),
            "data: [DONE]\n".to_string(),
        ];
        let backend = MockStreamBackend { chunks };
        let mut deltas = Vec::new();

        let content = stream_prompt(
            &config(),
            "systeme",
            "Crée un niveau",
            &backend,
            &mut |d| deltas.push(d.to_string()),
            &|| false,
        )
        .expect("stream ok");

        assert_eq!(content, payload);
        assert_eq!(deltas.len(), 2);

        let parsed = AiClient::parse_llm_json_response(&content).expect("payload valide");
        assert_eq!(parsed.title, "Mock Stream");
    }

    #[test]
    fn stream_prompt_reports_cancellation() {
        let backend = MockStreamBackend {
            chunks: vec![
                "data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\n".to_string(),
                "data: {\"choices\":[{\"delta\":{\"content\":\"y\"}}]}\n".to_string(),
            ],
        };
        let calls = std::cell::Cell::new(0);
        let err = stream_prompt(
            &config(),
            "systeme",
            "intent",
            &backend,
            &mut |_d| {},
            &|| {
                calls.set(calls.get() + 1);
                calls.get() > 1 // annule après le premier chunk
            },
        )
        .expect_err("doit être annulé");

        assert!(err.contains("annulée"), "message : {err}");
    }
}

