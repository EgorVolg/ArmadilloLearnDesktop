use serde::{Deserialize, Serialize};
use serde_json;
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationResult {
    pub sentence_translation: String,
    pub word_translation: String,
    pub synonyms: Vec<String>,
    pub part_of_speech: String, // e.g., "noun", "verb", "adjective"
    pub topic: String,          // e.g., "Programming", "Animals", "Nature"
}

/// Запрашивает перевод, часть речи и тему у Groq API.
#[tauri::command]
pub async fn translate(full_text: &str, word: &str) -> Result<TranslationResult, String> {
    let api_key = env::var("GROQ_API_KEY").map_err(|_| "GROQ_API_KEY not set".to_string())?;

    let url = "https://api.groq.com/openai/v1/chat/completions";

    let client = reqwest::Client::new();
    let prompt = format!(
        "Given the English sentence and a specific word from it, provide:\n\
        1. Translation of the whole sentence into Russian.\n\
        2. Translation of the specific word into Russian according to the context.\n\
        3. Synonyms of the specific word in English (at least 2).\n\
        4. Part of speech of the specific word (in English).\n\
        5. Topic or category of the sentence (in English). Choose a concise label like Programming, Animals, Nature, Technology, etc.\n\
        \n\
        Sentence: \"{}\"\n\
        Word: \"{}\"\n\
        \n\
        Respond with valid JSON only, no extra text:\n\
        {{\n\
          \"sentence_translation\": \"...\",\n\
          \"word_translation\": \"...\",\n\
          \"synonyms\": [\"...\", \"...\"],\n\
          \"part_of_speech\": \"...\",\n\
          \"topic\": \"...\"\n\
        }}",
        full_text, word
    );

    let body = serde_json::json!({
        "model": "llama-3.3-70b-versatile",
        "messages": [
            {"role": "system", "content": "You are a translator and linguistic analyzer. CRITICAL: Output ONLY valid JSON. Do NOT include any thinking, reasoning, markdown formatting, or any text before or after the JSON object. Start directly with '{' and end with '}'. No code fences, no explanations."},
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.1,
        "max_tokens": 500
    });

    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let data: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let content = data["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("{}");

    // Extract JSON from the response, handling:
    // - <think>...</think> blocks (Qwen model reasoning)
    // - ```json ... ``` markdown code fences
    // - Extra text before/after JSON
    let cleaned = {
        // Remove all <think>...</think> blocks
        let mut s = content.to_string();
        while let (Some(start), Some(end)) = (s.find("<think>"), s.find("</think>")) {
            s.replace_range(start..=end + "</think>".len().saturating_sub(1), "");
        }

        // Try to find and parse a JSON object, looking at each '{' position
        let trimmed = s.trim();
        let mut best: Option<String> = None;

        // Strategy 1: find a substring from '{' to matching '}' that parses as JSON
        for (i, _) in trimmed.match_indices('{') {
            if let Some(end) = trimmed[i..].rfind('}') {
                let candidate_slice = &trimmed[i..=i + end];
                // Clean up any markdown fences inside
                let clean_json = candidate_slice
                    .replace("```json", "")
                    .replace("```", "")
                    .trim()
                    .to_string();
                if serde_json::from_str::<serde_json::Value>(&clean_json).is_ok() {
                    best = Some(clean_json);
                    break;
                }
            }
        }

        // Strategy 2: if no JSON found, try removing all non-JSON characters and retry
        let best = best.unwrap_or_else(|| {
            // Last resort: return empty JSON object
            "{}".to_string()
        });

        best
    };

    let parsed: serde_json::Value = serde_json::from_str(&cleaned).map_err(|e| {
        eprintln!("Invalid JSON from AI. Raw content:\n---\n{}\n---", content);
        eprintln!("Cleaned content:\n---\n{}\n---", cleaned);
        format!("Invalid JSON from AI: {}", e)
    })?;

    Ok(TranslationResult {
        sentence_translation: parsed["sentence_translation"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        word_translation: parsed["word_translation"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        synonyms: parsed["synonyms"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        part_of_speech: parsed["part_of_speech"].as_str().unwrap_or("").to_string(),
        topic: parsed["topic"].as_str().unwrap_or("").to_string(),
    })
}
