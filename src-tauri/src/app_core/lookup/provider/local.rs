use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;

use crate::app_core::lookup::{provider::_trait::AiProvider, types::LookupResult};

const OLLAMA_URL: &str = "http://localhost:11434/api/chat";

/// Модель можно переопределить без перекомпиляции:
/// ARMADILLO_OLLAMA_MODEL=qwen2.5vl:7b
///
/// По умолчанию 3b, а не 7b: на GPU с 6GB VRAM модель 7b (~6.2GB)
/// не помещается целиком, ~46% слоёв выполняется на CPU, и генерация
/// падает до ~12 tok/s (ответ за 9-17 секунд). 3b (~2.8GB) целиком
/// живёт в VRAM и выдаёт ~94 tok/s (ответ за ~1.5 секунды).
fn ollama_model() -> String {
    std::env::var("ARMADILLO_OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5vl:3b".to_string())
}

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

    /// Прогрев: загружает веса модели в VRAM сразу при старте приложения,
    /// чтобы первый lookup не ждал загрузки модели (десятки секунд).
    ///
    /// Возвращает true, если модель ответила (резидентна в VRAM).
    /// Ошибки намеренно не печатаются: ретраи выполняет runtime.rs,
    /// а при окончательной неудаче первый lookup покажет ошибку сам.
    pub fn warm_up(&self) -> bool {
        let started = Instant::now();

        let request = json!({
            "model": ollama_model(),
            "keep_alive": -1,
            "prompt": "ok",
            "stream": false,
            "options": {
                "num_predict": 1
            }
        });

        let result = self
            .client
            .post("http://localhost:11434/api/generate")
            .json(&request)
            .send();

        match result {
            Ok(response) if response.status().is_success() => {
                println!(
                    "AI model warm-up finished in {:.2} s",
                    started.elapsed().as_secs_f64()
                );

                true
            }
            _ => false,
        }
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
    meaning: String,
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
            r#"Analyze the English word "{word}" in context.

Context:
{sentence}

Rules:
- meaning: concise definition of the word's meaning in this context, in one sentence.
- word: copy the target word exactly.
- sentence_translation: natural, fluent Russian translation of the full sentence.
- word_translation: the most appropriate Russian translation of the word in this context.
- synonyms: 2-4 natural English synonyms matching this meaning.
- part_of_speech: grammatical category of the word in this sentence.
- topic: 1-3 word description of the sentence topic.
- Use the context to resolve ambiguity.
- Do not invent meanings unsupported by the context.
- Return only the JSON object. No explanations or markdown."#
        );

        let format = json!({
            "type": "object",
            "properties": {
                "meaning": {
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
                "meaning",
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
            "model": ollama_model(),
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

                // Промпт + ответ укладываются в ~500 токенов. Маленький
                // контекст = маленький KV-кэш = больше слоёв помещается
                // в VRAM вместо CPU-offload.
                "num_ctx": 2048,

                // 80 is too small and causes truncated JSON.
                "num_predict": 256
            }
        });

        let request_started = Instant::now();

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

        println!(
            "Ollama response занял {:.2} s",
            request_started.elapsed().as_secs_f64()
        );

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
            meaning: generated.meaning.to_string(),
            sentence_translation: generated.sentence_translation,
            word_translation: generated.word_translation,
            synonyms: generated.synonyms,
            part_of_speech: generated.part_of_speech,
            topic: generated.topic,
        })
    }
}
