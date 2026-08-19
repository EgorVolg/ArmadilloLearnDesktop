use std::time::Duration;

use base64::{ engine::general_purpose, Engine as _ };
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::app_core::lookup::{ LookupResult, provider::_trait::AiProvider };

const GROQ_URL: &str = "https://api.groq.com/openai/v1/chat/completions";

const GROQ_MODEL: &str = "qwen/qwen3.6-27b";

pub struct GroqProvider {
    client: Client,
}

impl GroqProvider {
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|error| { format!("Failed to create HTTP client: {error}") })?;

        Ok(Self { client })
    }

    fn api_key() -> Result<String, String> {
        std::env
            ::var("GROQ_API_KEY")
            .map_err(|_| { "GROQ_API_KEY environment variable is not set".to_string() })
    }

    fn system_prompt() -> &'static str {
        r#"
You are an English language learning assistant.

Look ONLY at the provided image.

There is a yellow marker with a small cross on the image.
The center of this yellow marker indicates the exact text
the user selected.

Your task is extremely simple:

1. Find the yellow marker.
2. Look directly underneath its CENTER.
3. Identify the smallest meaningful English word or short
   phrase located there.
4. Translate that word or phrase into natural Russian.
5. Use the surrounding visible text to provide the sentence
   or line containing the selected word.

IMPORTANT:

- The yellow marker in the image is the ONLY indication of
  what the user selected.
- Do NOT infer the selected word from the general topic.
- Do NOT choose a nearby word just because it is more
  semantically interesting.
- Do NOT choose text from somewhere else in the image.
- Do NOT choose a word merely because it appears near the
  marker.
- The selected word must be the text physically located
  directly underneath the CENTER of the yellow marker.
- Ignore the yellow marker itself; it is not text.
- The image may contain programming code, terminal output,
  identifiers, warnings, UI labels, or normal English text.
  All of these are valid targets.
- Do not use coordinates.
- Do not ask for coordinates.
- Coordinates are irrelevant.
- Do not mention coordinates in the answer.
- The answer MUST be based only on text visibly present in
  the image.

For programming code:

- Keep the original code line in "sentence".
- Explain its meaning naturally in Russian in
  "sentence_translation".
- Translate the selected English identifier according to
  its normal English meaning when possible.

Return exactly one JSON object.

The object MUST contain exactly these fields:

{
  "sentence": "",
  "word": "",
  "sentence_translation": "",
  "word_translation": "",
  "synonyms": [],
  "part_of_speech": "",
  "topic": ""
}

Return JSON only.
Do not use markdown.
Do not include explanations outside the JSON object.
"#
    }
}

#[derive(Debug, Deserialize)]
struct GroqResponse {
    choices: Vec<GroqChoice>,
}

#[derive(Debug, Deserialize)]
struct GroqChoice {
    message: GroqMessage,
}

#[derive(Debug, Deserialize)]
struct GroqMessage {
    content: Option<String>,
}

impl AiProvider for GroqProvider {
    fn lookup(&self, image_png: &[u8], prompt: &str) -> Result<LookupResult, String> {
        let api_key = Self::api_key()?;

        let encoded = general_purpose::STANDARD.encode(image_png);

        let image_url = format!("data:image/png;base64,{encoded}");

        let request =
            serde_json::json!({
            "model": GROQ_MODEL,

            "messages": [
                {
                    "role": "system",
                    "content": Self::system_prompt()
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": "Translate the English word or short phrase directly under the center of the yellow marker."
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": image_url
                            }
                        }
                    ]
                }
            ],

            "temperature": 0.0,
            "max_completion_tokens": 500,
            "reasoning_effort": "none",

            "response_format": {
                "type": "json_object"
            }
        });

        println!("Sending screenshot to Groq...");

        let response = self.client
            .post(GROQ_URL)
            .bearer_auth(api_key)
            .json(&request)
            .send()
            .map_err(|error| { format!("Groq request failed: {error}") })?;

        let status = response.status();

        println!("Groq HTTP status: {status}");

        let response_text = response
            .text()
            .map_err(|error| { format!("Failed to read Groq response: {error}") })?;

        if !status.is_success() {
            return Err(format!("Groq API returned {status}: {response_text}"));
        }

        println!("=== GROQ API RESPONSE ===");
        println!("{response_text}");
        println!("=== END GROQ API RESPONSE ===");

        let groq_response: GroqResponse = serde_json
            ::from_str(&response_text)
            .map_err(|error| { format!("Failed to parse Groq API response: {error}") })?;

        let content = groq_response.choices
            .first()
            .and_then(|choice| choice.message.content.as_ref())
            .ok_or_else(|| { "Groq response contains no message content".to_string() })?;

        println!("=== GROQ CONTENT ===");
        println!("{content}");
        println!("=== END GROQ CONTENT ===");

        serde_json
            ::from_str(content)
            .map_err(|error| {
                format!("Failed to parse lookup JSON: {}\nContent: {}", error, content)
            })
    }
}
