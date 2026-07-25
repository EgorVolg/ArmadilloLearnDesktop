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
            {"role": "system", "content": "You are a translator and linguistic analyzer. Always reply with JSON only."},
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.3,
        "max_tokens": 250
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

    let parsed: serde_json::Value =
        serde_json::from_str(content).map_err(|e| format!("Invalid JSON from AI: {}", e))?;

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
