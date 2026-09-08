use std::time::Duration;

use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use super::local::LocalProvider;
use crate::app_core::lookup::provider::_trait::AiProvider;
use crate::app_core::lookup::provider::local::{
    align_word_translation, canonicalize_topic, clean_synonyms, contains_cyrillic,
    is_transliteration, lookup_format, strip_json_fences, synonyms_to_vec, system_prompt,
    topic_to_string, word_translation_is_connected,
};
use crate::app_core::lookup::types::LookupResult;

const GROQ_URL: &str = "https://api.groq.com/openai/v1/chat/completions";

/// Быстрая модель free-tier Groq: сотни tok/s, structured outputs,
/// reasoning_effort. Переопределяется ARMADILLO_GROQ_MODEL.
const DEFAULT_MODEL: &str = "openai/gpt-oss-120b";

/// Полный бюджет одного lookup через облако. При превышении —
/// ошибка и автоматический fallback на локальную Ollama.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(12);

/// Облачный провайдер Groq (OpenAI-совместимый API).
///
/// Ключ берётся из GROQ_API_KEY; без ключа new() возвращает Err —
/// runtime в этом случае просто не создаёт облачный путь.
pub struct GroqProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl GroqProvider {
    pub fn new() -> Result<Self, String> {
        let api_key = std::env::var("GROQ_API_KEY")
            .map(|value| value.trim().to_string())
            .ok()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "GROQ_API_KEY is not set".to_string())?;

        let model = std::env::var("ARMADILLO_GROQ_MODEL")
            .map(|value| value.trim().to_string())
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());

        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| format!("failed to build HTTP client: {error}"))?;

        Ok(Self {
            client,
            api_key,
            model,
        })
    }

    /// Один chat-completions вызов с той же JSON-схемой и системным
    /// промптом, что и у локального провайдера — правила (русский язык,
    /// enum-топик, форма слова) идентичны.
    fn chat(&self, messages: &Value) -> Result<String, String> {
        let body = json!({
            "model": &self.model,
            "messages": messages,
            "temperature": 0.2,
            "max_completion_tokens": 1024,
            "reasoning_effort": "low",
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "lookup",
                    "strict": true,
                    "schema": lookup_format(),
                },
            },
        });

        let response = self
            .client
            .post(GROQ_URL)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .map_err(|error| format!("Groq request failed: {error}"))?;

        let status = response.status();

        if !status.is_success() {
            let body = response.text().unwrap_or_default();

            let hint = match status.as_u16() {
                401 | 403 => " — проверь GROQ_API_KEY и что VPN пропускает этот трафик (RU-IP блокируется)",
                429 => " — превышен лимит free-tier Groq",
                _ => "",
            };

            let snippet: String = body.chars().take(200).collect();

            return Err(format!("Groq HTTP {status}{hint}: {snippet}"));
        }

        let parsed: GroqResponse = response
            .json()
            .map_err(|error| format!("Groq: invalid response JSON: {error}"))?;

        parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or_else(|| "Groq: empty choices".to_string())
    }

    /// chat + разбор ответа модели. Возвращает структуру и сырой контент
    /// (контент нужен корректирующим повторам как реплика assistant).
    fn request_lookup(&self, messages: Value) -> Result<(GroqLookup, String), String> {
        let content = self.chat(&messages)?;

        let cleaned = strip_json_fences(&content);

        let parsed: GroqLookup = serde_json::from_str(&cleaned)
            .map_err(|error| format!("Groq: failed to parse lookup JSON: {error}"))?;

        Ok((parsed, content))
    }
}

// =========================================================
// API ENVELOPE
// =========================================================

#[derive(Deserialize)]
struct GroqResponse {
    choices: Vec<GroqChoice>,
}

#[derive(Deserialize)]
struct GroqChoice {
    message: GroqMessage,
}

#[derive(Deserialize)]
struct GroqMessage {
    content: Option<String>,
}

// =========================================================
// MODEL OUTPUT
// =========================================================

/// Поля ответа модели — те же, что у локального провайдера (LocalLookup),
/// поэтому пост-обработка (align/clean/canonicalize) полностью идентична.
/// Все поля с default: деградация пустым значением вместо падения.
#[derive(Debug, Deserialize)]
struct GroqLookup {
    #[serde(default)]
    sentence_translation: String,

    #[serde(default)]
    word_translation: String,

    #[serde(default)]
    meaning: String,

    #[serde(default)]
    synonyms: serde_json::Value,

    #[serde(default)]
    part_of_speech: String,

    #[serde(default)]
    topic: serde_json::Value,
}

// =========================================================
// AI PROVIDER IMPL
// =========================================================

impl AiProvider for GroqProvider {
    fn lookup(&self, sentence: &str, word: &str) -> Result<LookupResult, String> {
        println!("[lookup/groq] word=\"{word}\", model={}", self.model);

        let messages = json!([
            {
                "role": "system",
                "content": &system_prompt()
            },
            {
                "role": "user",
                "content": format!("Target word: \"{word}\"\n\nContext sentence:\n{sentence}")
            }
        ]);

        let (mut generated, content) = self.request_lookup(messages)?;

        // Гарантия «перевод приходит на русском» — те же корректирующие
        // повторы, что у локального провайдера (см. local.rs): правило
        // и формулировка идентичны, различается только транспорт.
        if !contains_cyrillic(&generated.sentence_translation) {
            println!("sentence_translation came back without Russian text, retrying once");

            let corrective = json!([
                {
                    "role": "system",
                    "content": &system_prompt()
                },
                {
                    "role": "user",
                    "content": format!("Target word: \"{word}\"\n\nContext sentence:\n{sentence}")
                },
                {
                    "role": "assistant",
                    "content": &content
                },
                {
                    "role": "user",
                    "content": "Your sentence_translation was not in Russian. Reply with the same JSON object again, but sentence_translation must be the complete Russian translation of the context sentence."
                }
            ]);

            match self.request_lookup(corrective) {
                Ok((retried, _)) => generated = retried,
                Err(retry_error) => println!("Correction retry failed: {retry_error}"),
            }
        }

        if is_transliteration(word, &generated.word_translation)
            || is_transliteration(word, &generated.sentence_translation)
        {
            println!("transliteration detected for word '{word}', retrying once");

            let corrective = json!([
                {
                    "role": "system",
                    "content": &system_prompt()
                },
                {
                    "role": "user",
                    "content": format!("Target word: \"{word}\"\n\nContext sentence:\n{sentence}")
                },
                {
                    "role": "assistant",
                    "content": &content
                },
                {
                    "role": "user",
                    "content": "Your reply transliterated the English word into Russian letters (like \"Армадилло\" for \"armadillo\"). Such words do not exist in Russian. Reply with the same JSON object again, but word_translation and sentence_translation must use the REAL Russian word (\"armadillo\" is \"броненосец\"). Never write English words in Cyrillic letters."
                }
            ]);

            match self.request_lookup(corrective) {
                Ok((retried, _)) => generated = retried,
                Err(retry_error) => println!("Transliteration retry failed: {retry_error}"),
            }
        }

        // Валидация согласованности — те же корректирующие повторы, что у
        // локального провайдера (см. local.rs): правило и формулировка
        // идентичны, различается только транспорт.
        if !word_translation_is_connected(
            &generated.word_translation,
            &generated.sentence_translation,
        ) {
            println!(
                "word_translation '{}' is unrelated to sentence_translation, retrying once",
                generated.word_translation
            );

            let corrective = json!([
                {
                    "role": "system",
                    "content": &system_prompt()
                },
                {
                    "role": "user",
                    "content": format!("Target word: \"{word}\"\n\nContext sentence:\n{sentence}")
                },
                {
                    "role": "assistant",
                    "content": &content
                },
                {
                    "role": "user",
                    "content": format!("Your word_translation is unrelated to your sentence_translation. The target word is \"{word}\". Reply with the same JSON object again, but word_translation must be the natural Russian translation of \"{word}\" as used in the context sentence, in the same grammatical form as it appears inside sentence_translation, so that this exact string occurs inside sentence_translation. The meaning must explain \"{word}\" itself, not another word from the sentence.")
                }
            ]);

            match self.request_lookup(corrective) {
                Ok((retried, _)) => generated = retried,
                Err(retry_error) => println!("Consistency retry failed: {retry_error}"),
            }
        }

        let sentence_translation = generated.sentence_translation;

        let word_translation =
            align_word_translation(&generated.word_translation, &sentence_translation);

        Ok(LookupResult {
            word: word.to_string(),
            meaning: generated.meaning,
            sentence_translation,
            word_translation,
            synonyms: clean_synonyms(word, synonyms_to_vec(&generated.synonyms)),
            part_of_speech: generated.part_of_speech,
            topic: canonicalize_topic(&topic_to_string(&generated.topic)),
        })
    }
}

// ============================================================================
// Гибрид: Groq основной, локальная Ollama — fallback
// ============================================================================

/// Облачный Groq как основной источник, локальная Ollama — страховка.
///
/// Groq недоступен по множеству независимых причин (RU-IP без VPN,
/// исчерпанный free-лимит 429, таймаут, сбой сети, смена модели на стороне
/// API), поэтому ЛЮБАЯ ошибка primary перекладывается на local:
/// пользователь всегда получает карточку, worst case — как раньше.
pub struct HybridProvider {
    primary: Box<dyn AiProvider>,
    fallback: Box<dyn AiProvider>,
}

impl HybridProvider {
    pub fn new(primary: Box<dyn AiProvider>, fallback: Box<dyn AiProvider>) -> Self {
        Self { primary, fallback }
    }
}

impl AiProvider for HybridProvider {
    fn lookup(&self, context: &str, clicked_word: &str) -> Result<LookupResult, String> {
        match self.primary.lookup(context, clicked_word) {
            Ok(result) => Ok(result),
            Err(error) => {
                println!("[provider] Groq недоступен ({error}) — fallback на локальную Ollama");

                self.fallback.lookup(context, clicked_word)
            }
        }
    }
}

// ============================================================================
// Фабрика провайдера
// ============================================================================

/// Выбор провайдера по окружению:
///
/// - `GROQ_API_KEY` задан → гибрид «Groq + локальная Ollama»;
/// - ключа нет → только локальная Ollama (прежнее поведение);
/// - `ARMADILLO_PROVIDER=local` принудительно выключает Groq даже при ключе.
pub fn build_provider() -> std::sync::Arc<dyn AiProvider> {
    let local: Box<dyn AiProvider> =
        Box::new(LocalProvider::new().expect("failed to init local Ollama provider"));

    let groq_allowed = matches!(
        std::env::var("ARMADILLO_PROVIDER").ok().as_deref(),
        None | Some("") | Some("groq")
    );

    let groq = if groq_allowed {
        // Нет ключа / битый клиент — просто остаёмся на локальной модели.
        GroqProvider::new().ok()
    } else {
        None
    };

    match groq {
        Some(groq) => {
            println!(
                "[provider] primary: Groq ({}) | fallback: local Ollama",
                groq.model
            );

            std::sync::Arc::new(HybridProvider::new(Box::new(groq), local))
        }
        None => {
            println!("[provider] primary: local Ollama (GROQ_API_KEY не задан)");

            std::sync::Arc::from(local)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubPrimaryOk;

    impl AiProvider for StubPrimaryOk {
        fn lookup(&self, _context: &str, _clicked_word: &str) -> Result<LookupResult, String> {
            Ok(LookupResult {
                word: "primary".to_string(),
                sentence_translation: "Предложение от primary.".to_string(),
                word_translation: "слово".to_string(),
                meaning: "It means the thing from primary.".to_string(),
                synonyms: vec!["near".to_string()],
                part_of_speech: "noun".to_string(),
                topic: "Работа и профессии".to_string(),
            })
        }
    }

    struct StubAlwaysFails {
        message: &'static str,
    }

    impl AiProvider for StubAlwaysFails {
        fn lookup(&self, _context: &str, _clicked_word: &str) -> Result<LookupResult, String> {
            Err(self.message.to_string())
        }
    }

    /// Primary ответил — fallback не должен вызываться вовсе
    /// (он в тесте всегда падает: если бы вызывался, результат был бы Err).
    #[test]
    fn hybrid_uses_primary_without_touching_fallback() {
        let hybrid = HybridProvider::new(
            Box::new(StubPrimaryOk),
            Box::new(StubAlwaysFails {
                message: "fallback был вызван — так нельзя",
            }),
        );

        let result = hybrid.lookup("Some context.", "word").expect("primary ok");

        assert_eq!(result.word, "primary");
    }

    /// Primary упал → ответ должен прийти от fallback.
    #[test]
    fn hybrid_falls_back_when_primary_errors() {
        let hybrid = HybridProvider::new(
            Box::new(StubAlwaysFails {
                message: "Groq offline (403 за RU-IP)",
            }),
            Box::new(StubPrimaryOk),
        );

        let result = hybrid.lookup("Some context.", "word").expect("fallback ok");

        assert_eq!(result.word, "primary");
    }

    /// Живой прогон Groq: требует GROQ_API_KEY и системный VPN
    /// (запросы с RU-IP Groq отклоняет с 403 — см. build_provider).
    ///
    /// Запуск:
    ///   cargo test --lib -- --ignored real_lookup_groq_smoke --nocapture
    #[test]
    #[ignore = "требует GROQ_API_KEY и VPN: реальный вызов Groq"]
    fn real_lookup_groq_smoke() {
        let provider = GroqProvider::new().expect("нет GROQ_API_KEY или клиент не собрался");

        let started = std::time::Instant::now();

        let result = provider
            .lookup("More instructions", "more")
            .expect("lookup должен отработать");

        println!("lookup took {:.2} s", started.elapsed().as_secs_f64());
        println!("sentence_translation: {}", result.sentence_translation);
        println!("word_translation: {}", result.word_translation);
        println!("meaning: {}", result.meaning);
        println!("topic: {}", result.topic);
        println!("part_of_speech: {}", result.part_of_speech);
        println!("synonyms: {:?}", result.synonyms);

        let has_cyrillic = result
            .sentence_translation
            .chars()
            .any(|ch| ('\u{0400}'..='\u{04FF}').contains(&ch));

        assert!(has_cyrillic, "перевод предложения должен быть на русском");
    }
}