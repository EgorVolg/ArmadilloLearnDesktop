use std::time::Duration;

use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;

use crate::app_core::lookup::{provider::_trait::AiProvider, types::LookupResult};

const OLLAMA_URL: &str = "http://localhost:11434/api/chat";
const OLLAMA_MODEL: &str = "qwen2.5vl:7b";

pub struct LocalProvider {
    client: Client,
}

impl LocalProvider {
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(60))
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
}

#[derive(Debug, Deserialize)]
struct LocalLookup {
    sentence: String,
    word: String,
    sentence_translation: String,
    word_translation: String,
    synonyms: Vec<String>,
    part_of_speech: String,
    topic: String,
}

impl AiProvider for LocalProvider {
    fn lookup(&self, sentence: &str, word: &str) -> Result<LookupResult, String> {
        let prompt = format!(
            r#"Analyze the English word "{word}" in this sentence:

{sentence}

Return the result as JSON matching the provided schema.

Rules:
- sentence: copy the English sentence exactly.
- word: copy the target word exactly.
- sentence_translation: natural Russian translation of the sentence.
- word_translation: Russian translation of the target word in this context.
- synonyms: exactly 2-4 English synonyms.
- part_of_speech: grammatical category.
- topic: short topic describing the context.
- Do not add explanations.
- Do not use markdown."#
        );

        let format = json!({
            "type": "object",
            "properties": {
                "sentence": {
                    "type": "string"
                },
                "word": {
                    "type": "string"
                },
                "sentence_translation": {
                    "type": "string"
                },
                "word_translation": {
                    "type": "string"
                },
                "synonyms": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "minItems": 2,
                    "maxItems": 4
                },
                "part_of_speech": {
                    "type": "string"
                },
                "topic": {
                    "type": "string"
                }
            },
            "required": [
                "sentence",
                "word",
                "sentence_translation",
                "word_translation",
                "synonyms",
                "part_of_speech",
                "topic"
            ],
            "additionalProperties": false
        });

        let request = json!({
            "model": OLLAMA_MODEL,
            "keep_alive": -1,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "stream": false,
            "think": false,

            // IMPORTANT:
            // JSON Schema instead of just "json".
            "format": format,

            "options": {
                "temperature": 0.0,

                // 80 is too small and causes truncated JSON.
                "num_predict": 256
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
            .ok_or_else(|| "Ollama returned empty content".to_string())?;

        let generated: LocalLookup = serde_json::from_str(content)
            .map_err(|error| format!("Failed to parse lookup JSON: {error}\nContent: {content}"))?;

        Ok(LookupResult {
            word: word.to_string(),
            sentence: sentence.to_string(),
            sentence_translation: generated.sentence_translation,
            word_translation: generated.word_translation,
            synonyms: generated.synonyms,
            part_of_speech: generated.part_of_speech,
            topic: generated.topic,
        })
    }
}
