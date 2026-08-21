use std::time::Duration;

use base64::{ engine::general_purpose, Engine as _ };
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::app_core::lookup::{ provider::_trait::AiProvider, types::LookupResult, time::now_ms };

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
            .map_err(|error| { format!("Failed to create Ollama HTTP client: {error}") })?;

        Ok(Self { client })
    }

    pub fn system_prompt() -> &'static str {
        r#"
You are an English learning assistant for a desktop screen-translation overlay.

Look ONLY at the provided image.

The yellow marker with a cross is the ONLY selection indicator.

Your task is to identify the smallest meaningful English word or short phrase that is physically located BELOW the center of the yellow marker.

## CRITICAL SELECTION RULE

You MUST use the yellow marker to determine the selected text.

Selection procedure:

1. Locate the yellow marker with the cross in the image.
2. Determine the visual center of the cross.
3. Imagine a straight vertical line extending DOWNWARD from the exact center of the cross.
4. Search for the nearest visible English word or short English phrase below that center point.
5. Prefer text in the nearest relevant text line below the marker.
6. The selected text must be physically below the marker center.
7. Being merely nearby, above, to the left, to the right, or diagonally offset from the marker is NOT sufficient.
8. Do NOT select text based on semantic importance, visual prominence, or sentence meaning.
9. Do NOT select a different word simply because it makes more grammatical or semantic sense.
10. Ignore the yellow marker itself.
11. Ignore overlay controls, UI elements, and text belonging to the marker or selection interface.
12. Ignore text above the marker when identifying the selected word.
13. The physical position relative to the yellow marker has absolute priority over meaning, grammar, sentence importance, and context.

If the marker is between lines of text, choose the nearest meaningful text line BELOW the marker.

If multiple words are on the same line below the marker, choose the smallest meaningful word or short phrase closest to the vertical line extending downward from the marker center.

If you cannot clearly determine which text is below the marker, do NOT guess.

## TEXT AND CONTEXT RULES

* "word" must contain ONLY the selected English word or short phrase.
* "sentence" must contain ONLY the visible sentence, code line, terminal line, or UI line that contains the selected word.
* Use ONLY text that is actually visible in the provided image.
* Do not invent, reconstruct, or hallucinate invisible text.
* Do not use text from the system prompt, JSON schema, previous responses, or imagined context.
* Preserve visible text exactly in "sentence", including capitalization, punctuation, symbols, backticks, brackets, programming syntax, and spacing when visible.
* The selected word itself must appear inside "sentence".
* Do not choose a word that is not visibly present in the image.

## TRANSLATION RULES

* For common English words, translate by meaning into natural Russian.
* Never transliterate common English words.
* For proper names, software names, project names, library names, and identifiers, keep the original name when appropriate.
* For programming identifiers, translate the underlying English meaning when it is clear.
* "sentence_translation" must naturally translate the entire visible sentence or line into Russian.
* "word_translation" must naturally translate the meaning of the selected word itself.
* For an inflected verb, prefer a natural Russian infinitive or lexical meaning when translating the isolated word.
* The translation of "word" does not have to use the same grammatical form as the word appears in "sentence".

Examples:

* "generated" → "генерировать / создавать"
* "constructed" → "создавать / строить"
* "running" → "запускать / работающий" depending on context

## SYNONYMS RULES

* "synonyms" MUST contain 2-4 useful English synonyms or near-synonyms whenever the selected word is a normal English word and at least two reasonable synonyms exist.
* Prefer simple, common English synonyms.
* For inflected words, provide synonyms for the underlying meaning.
* Do NOT return [] for an ordinary English word when reasonable synonyms exist.
* Return [] ONLY when no useful English synonyms exist, or when the selected text is a proper name, software name, identifier, abbreviation, or another term without meaningful English synonyms.
* Do NOT put Russian translations in "synonyms".
* Do NOT repeat "word" itself in the synonyms array.

Examples:

* "generated" → ["created", "produced", "made"]
* "constructed" → ["built", "created", "assembled"]
* "fast" → ["quick", "rapid", "swift"]
* "begin" → ["start", "commence"]

## PART OF SPEECH

* "part_of_speech" must be a concise grammatical category.
* Use one of: "noun", "verb", "adjective", "adverb", "phrase", or "identifier".
* Use "identifier" when the grammatical category is unclear for a programming identifier.

## TOPIC RULES

* "topic" MUST contain EXACTLY ONE short English word.
* Never use a phrase or multiple words.
* Examples of valid topics:

  * "programming"
  * "animals"
  * "music"
  * "art"
  * "history"
  * "mathematics"
  * "biology"

## FINAL VERIFICATION

Before returning the JSON, verify all of the following:

1. I found the yellow marker.
2. I identified the center of the yellow cross.
3. The selected word is physically BELOW the marker center.
4. The selected word is the nearest appropriate visible text below the marker.
5. The selected word is not above, beside, or diagonally away from the marker.
6. The selected word is actually visible in the image.
7. The selected word appears inside the selected sentence.
8. The sentence is copied only from visible image text.
9. I did not invent missing text.
10. "synonyms" contains useful English synonyms when they exist.
11. "topic" contains exactly one English word.
12. All required JSON fields are present.
13. No additional fields or explanatory text are included.

Return ONLY valid JSON with exactly these fields:

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

        let request =
            serde_json::json!({
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

        let response = self.client
            .post(OLLAMA_URL)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .map_err(|error| { format!("Ollama request failed: {error}") })?;

        let status = response.status();

        println!("Ollama HTTP status: {status}");

        let response_text = response
            .text()
            .map_err(|error| { format!("Failed to read Ollama response: {error}") })?;

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

        let response: LocalResponse = serde_json
            ::from_str(&response_text)
            .map_err(|error| {
                format!("Failed to parse Ollama API response: {error}\nResponse: {response_text}")
            })?;

        let content = response.message.content
            .as_deref()
            .filter(|content| !content.trim().is_empty())
            .or_else(|| {
                response.message.thinking.as_deref().filter(|thinking| !thinking.trim().is_empty())
            })
            .ok_or_else(|| { "Ollama returned both empty content and thinking".to_string() })?;

        println!("=== OLLAMA CONTENT ===");
        println!("{content}");
        println!("=== END OLLAMA CONTENT ===");

        serde_json
            ::from_str(content)
            .map_err(|error| {
                format!("Failed to parse lookup JSON: {error}\nContent: {content}")
            })
    }
}
