use std::time::Duration;

use base64::{engine::general_purpose, Engine as _};
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::app_core::lookup::{provider::_trait::AiProvider, time::now_ms, types::LookupResult};

const OLLAMA_URL: &str = "http://localhost:11434/api/chat";
// const OLLAMA_MODEL: &str = "qwen3-vl:8b-instruct";
const OLLAMA_MODEL: &str = "qwen3-vl:4b-instruct";

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

    pub fn system_prompt() -> &'static str {
        r#"
You are an English-learning assistant for a desktop screen-translation overlay.

IMPORTANT:
Use ONLY the provided image.
Do NOT use outside knowledge to identify text that is not visible.
Return ONLY valid JSON with exactly the required fields.

## TASK

Find the yellow marker with a cross.

Use the visual CENTER of the yellow cross as the reference point.

Select the nearest meaningful English word or short English phrase that is physically BELOW the marker center.

Selection rules:
1. The selected text MUST be below the marker center.
2. Do NOT select text that is above, beside, or diagonally offset from the marker.
3. If the marker is between text lines, select the nearest suitable line BELOW it.
4. If several words are on the same line, select the smallest meaningful word or short phrase closest to the vertical line extending downward from the marker center.
5. Physical position is more important than meaning, grammar, sentence importance, or visual prominence.
6. Ignore the yellow marker itself.
7. If the target cannot be determined clearly, do NOT guess.

## TEXT RULES

"word" = ONLY the selected visible English word or short phrase.

"sentence" = ONLY the complete visible sentence, code line, terminal line, or UI line containing the selected word.

The selected word MUST visibly occur inside "sentence".

Copy visible text exactly in "sentence":
- preserve capitalization
- preserve punctuation
- preserve symbols
- preserve brackets
- preserve backticks
- preserve programming syntax
- preserve visible spacing when relevant

Never invent, reconstruct, or hallucinate missing text.

## LANGUAGE ANALYSIS

After identifying the selected word, complete ALL linguistic fields.

### TRANSLATION

"sentence_translation" = natural Russian translation of the entire visible sentence or line.

"word_translation" = natural Russian translation of the selected word itself.

For inflected verbs, prefer a natural Russian infinitive or lexical meaning.

Examples:
generated -> "генерировать / создавать"
constructed -> "создавать / строить"
running -> translate according to context

Common English words must be translated by meaning, not transliterated.

Proper names, software names, project names, library names, and identifiers may remain unchanged when appropriate.

### SYNONYMS

This field is REQUIRED for normal English words.

If "word" is a normal English lexical word:
- "synonyms" MUST contain 2-4 useful English synonyms or near-synonyms.
- NEVER return [] for a normal English word when reasonable synonyms exist.
- Do NOT include the selected word itself.
- Use simple, common English synonyms.
- For inflected forms, give synonyms for the underlying meaning.

Examples:
generated -> ["created", "produced", "made"]
constructed -> ["built", "created", "assembled"]
fast -> ["quick", "rapid", "swift"]
begin -> ["start", "commence"]

Return [] ONLY when:
- the selected text is a proper name,
- software/project/library name,
- abbreviation,
- programming identifier,
- or there is genuinely no reasonable English synonym.

IMPORTANT:
A word does NOT become an "identifier" merely because it appears inside source code.

For example, these are ordinary English words and require synonyms:
- generated
- running
- create
- start
- error
- build
- remove
- connect

Before returning JSON, verify:
- If the selected word is a normal English word, synonyms contains 2-4 English synonyms.
- If synonyms is [], there is a real reason why synonyms are not applicable.

### PART OF SPEECH

Use exactly one of:
- "noun"
- "verb"
- "adjective"
- "adverb"
- "phrase"
- "identifier"

Use "identifier" only when the selected text is genuinely a programming identifier or similar non-lexical identifier.

### TOPIC

"topic" MUST contain EXACTLY ONE short English word.

Never use a phrase or multiple words.

Examples:
programming
animals
music
art
history
mathematics
biology

Choose the single topic word that best describes the visible sentence or line.

## FINAL CHECK

Before returning JSON, silently verify all of the following:

1. I found the yellow marker.
2. I used the center of the yellow cross.
3. The selected text is physically BELOW the marker center.
4. The selected text is the nearest appropriate text below the marker.
5. The selected text is actually visible.
6. The selected word appears inside the selected sentence.
7. The sentence contains only visible image text.
8. I did not invent missing text.
9. sentence_translation translates the whole sentence.
10. word_translation translates the selected word.
11. If the word is a normal English word, synonyms contains 2-4 English synonyms.
12. synonyms does not contain Russian translations.
13. part_of_speech is one allowed value.
14. topic contains exactly ONE English word.
15. No extra fields are returned.

## OUTPUT

Return ONLY this JSON object:

{
  "sentence": "",
  "word": "",
  "sentence_translation": "",
  "word_translation": "",
  "synonyms": [],
  "part_of_speech": "",
  "topic": ""
}
"#
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
        println!("Sending screenshot to Ollama...");

        let encoded = general_purpose::STANDARD.encode(image_png);

        let request = serde_json::json!({
            "model": OLLAMA_MODEL,

            "messages": [
                {
                    "role": "system",
                    "content": LocalProvider::system_prompt()
                },
                {
                    "role": "user",
                    "content": prompt,
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

        let request_started = now_ms();

        let response = self
            .client
            .post(OLLAMA_URL)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .map_err(|error| format!("Ollama request failed: {error}"))?;

        let status = response.status();

        println!("Ollama HTTP status: {status}");

        let response_text = response
            .text()
            .map_err(|error| format!("Failed to read Ollama response: {error}"))?;

        let response_received = now_ms();

        println!(
            "Ollama round-trip: sent at {request_started} ms, response received at {response_received} ms (took {})",
            response_received.saturating_sub(request_started)
        );

        if !status.is_success() {
            return Err(format!("Ollama API returned {status}: {response_text}"));
        }

        println!("=== OLLAMA API RESPONSE ===");
        println!("{response_text}");
        println!("=== END OLLAMA API RESPONSE ===");

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

        println!("=== OLLAMA CONTENT ===");
        println!("{content}");
        println!("=== END OLLAMA CONTENT ===");

        serde_json::from_str(content)
            .map_err(|error| format!("Failed to parse lookup JSON: {error}\nContent: {content}"))
    }
}
