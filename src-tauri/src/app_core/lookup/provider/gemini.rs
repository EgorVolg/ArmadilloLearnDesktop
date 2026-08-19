use std::time::Duration;

use base64::{ engine::general_purpose, Engine as _ };
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;

use crate::app_core::lookup::{ provider::_trait::AiProvider, types::LookupResult };

const GEMINI_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";

const GEMINI_MODEL: &str = "gemini-3.5-flash";

pub struct GeminiProvider {
    client: Client,
}

impl GeminiProvider {
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|error| { format!("Failed to create Gemini HTTP client: {error}") })?;

        Ok(Self { client })
    }

    fn api_key() -> Result<String, String> {
        std::env
            ::var("GEMINI_API_KEY")
            .map_err(|_| "GEMINI_API_KEY environment variable is not set".to_string())
    }
}

impl AiProvider for GeminiProvider {
    fn lookup(&self, image_png: &[u8], prompt: &str) -> Result<LookupResult, String> {
        println!("Sending screenshot to Gemini...");

        let api_key = Self::api_key()?;

        let encoded_image = general_purpose::STANDARD.encode(image_png);

        let url = format!(
            "{}/{model}:generateContent?key={api_key}",
            GEMINI_URL,
            model = GEMINI_MODEL
        );

        let request =
            json!({
            "contents": [
                {
                    "parts": [
                        {
                            "text": prompt
                        },
                        {
                            "inline_data": {
                                "mime_type": "image/png",
                                "data": encoded_image
                            }
                        }
                    ]
                }
            ],

            "generationConfig": {
                "temperature": 0.0,
                "responseMimeType": "application/json",
                "responseSchema": {
                    "type": "OBJECT",
                    "properties": {
                        "sentence": {
                            "type": "STRING"
                        },
                        "word": {
                            "type": "STRING"
                        },
                        "sentence_translation": {
                            "type": "STRING"
                        },
                        "word_translation": {
                            "type": "STRING"
                        },
                        "synonyms": {
                            "type": "ARRAY",
                            "items": {
                                "type": "STRING"
                            }
                        },
                        "part_of_speech": {
                            "type": "STRING"
                        },
                        "topic": {
                            "type": "STRING"
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
                    ]
                }
            }
        });

        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .map_err(|error| { format!("Gemini request failed: {error}") })?;

        let status = response.status();

        println!("Gemini HTTP status: {}", status);

        let response_text = response
            .text()
            .map_err(|error| { format!("Failed to read Gemini response: {error}") })?;

        if !status.is_success() {
            return Err(format!("Gemini API returned {}: {}", status, response_text));
        }

        println!("=== GEMINI API RESPONSE ===");
        println!("{response_text}");
        println!("=== END GEMINI API RESPONSE ===");

        let gemini_response: GeminiResponse = serde_json
            ::from_str(&response_text)
            .map_err(|error| {
                format!(
                    "Failed to parse Gemini API response: {}\nResponse: {}",
                    error,
                    response_text
                )
            })?;

        let content = gemini_response.candidates
            .first()
            .and_then(|candidate| candidate.content.parts.first())
            .and_then(|part| part.text.as_deref())
            .ok_or_else(|| { "Gemini response contains no text content".to_string() })?;

        println!("=== GEMINI CONTENT ===");
        println!("{content}");
        println!("=== END GEMINI CONTENT ===");

        let result: LookupResult = serde_json
            ::from_str(content)
            .map_err(|error| {
                format!("Failed to parse Gemini lookup JSON: {}\nContent: {}", error, content)
            })?;

        println!("=== GEMINI LOOKUP SUCCESS ===");

        Ok(result)
    }
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
}

#[derive(Debug, Deserialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Deserialize)]
struct GeminiPart {
    text: Option<String>,
}
