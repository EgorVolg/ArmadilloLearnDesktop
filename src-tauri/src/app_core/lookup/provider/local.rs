use std::time::Duration;

use base64::{engine::general_purpose, Engine as _};
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::app_core::lookup::{provider::_trait::AiProvider, time::now_ms, types::LookupResult};

const OLLAMA_URL: &str = "http://localhost:11434/api/chat";
// const OLLAMA_MODEL: &str = "qwen3-vl:8b-instruct";
const OLLAMA_MODEL: &str = "qwen3-vl:8b-instruct";

pub struct LocalProvider {
    client: Client,
}

impl LocalProvider {
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|error| format!("Failed to create Ollama HTTP client: {error}"))?;

        Ok(Self { client })
    }
}

#[derive(Debug, Deserialize)]
struct LocalResponse {
    message: LocalMessage,
}

#[derive(Debug, Deserialize)]
struct LocalMessage {
    content: Option<String>,
    thinking: Option<String>,
}

impl AiProvider for LocalProvider {
    fn lookup(&self, image_png: &[u8], prompt: &str) -> Result<LookupResult, String> {
        let encoded = general_purpose::STANDARD.encode(image_png);

        let request = serde_json::json!({
            "model": OLLAMA_MODEL,
            "keep_alive": -1,

            "messages": [
                {
                    "role": "system",
                    "content": prompt
                },
                {
                    "role": "user",
                    "content": "",
                    "images": [encoded]
                }
            ],
            "stream": false,
            "think": false,
            "format": "json",

            "options": {
                "temperature": 0.0,
                "num_predict": 120
            }
        });

        let response = self
            .client
            .post(OLLAMA_URL)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .map_err(|error| format!("Ollama request failed: {error}"))?;

        let status = response.status();

        let response_text = response
            .text()
            .map_err(|error| format!("Failed to read Ollama response: {error}"))?;

        if !status.is_success() {
            return Err(format!("Ollama API returned {status}: {response_text}"));
        }

        let response: LocalResponse = serde_json::from_str(&response_text).map_err(|error| {
            format!("Failed to parse Ollama API response: {error}\nResponse: {response_text}")
        })?;

        let content = response
            .message
            .content
            .as_deref()
            .filter(|content| !content.trim().is_empty())
            .or_else(|| {
                response
                    .message
                    .thinking
                    .as_deref()
                    .filter(|thinking| !thinking.trim().is_empty())
            })
            .ok_or_else(|| "Ollama returned both empty content and thinking".to_string())?;

        serde_json::from_str(content)
            .map_err(|error| format!("Failed to parse lookup JSON: {error}\nContent: {content}"))
    }
}
