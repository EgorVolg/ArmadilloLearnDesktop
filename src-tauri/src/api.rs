use serde::{Deserialize, Serialize};
use serde_json;
use std::env;

fn extract_json(raw: &str) -> String {
    let mut s = raw.to_string();

    // Удаляем теги <thinking>...</thinking>, если есть
    loop {
        let start = s.find("<thinking>");
        let end = s.find("</thinking>");
        match (start, end) {
            (Some(ts), Some(te)) if te > ts => {
                s.replace_range(ts..=te + "</thinking>".len() - 1, "");
            }
            _ => break,
        }
    }

    let trimmed = s.trim();

    // Ищем первую валидную JSON-строку между { и }
    for (i, _) in trimmed.match_indices('{') {
        if let Some(end) = trimmed[i..].rfind('}') {
            let candidate = &trimmed[i..=i + end];
            let clean_json = candidate
                .replace("```json", "")
                .replace("```", "")
                .trim()
                .to_string();
            if serde_json::from_str::<serde_json::Value>(&clean_json).is_ok() {
                return clean_json;
            }
        }
    }

    // Если не нашли, возвращаем пустой объект
    "{}".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationResult {
    pub sentence: String,
    pub word: String,
    pub sentence_translation: String,
    pub word_translation: String,
    pub synonyms: Vec<String>,
    pub part_of_speech: String,
    pub topic: String,
}

pub fn fetch_translation_from_api(
    image1_base64: &str,
    mime1: &str,
    image2_base64: &str,
    mime2: &str,
) -> Result<TranslationResult, String> {
    let api_key = env::var("ZHIPU_API_KEY").map_err(|_| "ZHIPU_API_KEY not set".to_string())?;

    let url = "https://open.bigmodel.cn/api/paas/v4/chat/completions";
    let client = reqwest::blocking::Client::new();

    let data_uri1 = format!("data:{};base64,{}", mime1, image1_base64);
    let data_uri2 = format!("data:{};base64,{}", mime2, image2_base64);

    let prompt = r#"Analyze these two images and respond with ONLY a JSON object (no other text):

Image 1: fullscreen context. Image 2: zoomed word.

Tasks:
1. Identify the English word in Image 2.
2. Find the sentence in Image 1 containing that word.
3. Translate the sentence to Russian.
4. Translate the word to Russian (in context).
5. Provide 2 English synonyms.
6. Determine part of speech (noun/verb/adjective/adverb/etc).
7. Assign a topic (Programming/Medicine/Finance/Sports/etc).

JSON format:
{"sentence":"...","word":"...","sentence_translation":"...","word_translation":"...","synonyms":["...","..."],"part_of_speech":"...","topic":"..."}"#;

    let body = serde_json::json!({
        "model": "glm-5v-turbo",          // <-- новая модель
        "messages": [
            {
                "role": "system",
                "content": "You are a translator. Respond with valid JSON only."
            },
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": prompt},
                    {"type": "image_url", "image_url": {"url": data_uri1}},
                    {"type": "image_url", "image_url": {"url": data_uri2}}
                ]
            }
        ],
        "temperature": 0.1,
        "max_tokens": 1024,
        "thinking": {                    // <-- отключаем режим размышлений
            "type": "disabled"
        }
    });

    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| e.to_string())?;

    let data: serde_json::Value = response.json().map_err(|e| e.to_string())?;
    let content = data["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("{}");

    // Ваш существующий экстрактор JSON (чистка <thinking> и прочего)
    let cleaned = extract_json(content);

    let parsed: serde_json::Value = serde_json::from_str(&cleaned).map_err(|e| {
        eprintln!("Invalid JSON from AI: {}", e);
        format!("Invalid JSON: {}", e)
    })?;

    Ok(TranslationResult {
        sentence: parsed["sentence"].as_str().unwrap_or("").to_string(),
        word: parsed["word"].as_str().unwrap_or("").to_string(),
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
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        part_of_speech: parsed["part_of_speech"].as_str().unwrap_or("").to_string(),
        topic: parsed["topic"].as_str().unwrap_or("").to_string(),
    })
}
