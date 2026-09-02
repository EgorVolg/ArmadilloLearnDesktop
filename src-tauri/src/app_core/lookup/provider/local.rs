use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;

use crate::app_core::lookup::{provider::_trait::AiProvider, types::LookupResult};

const OLLAMA_URL: &str = "http://localhost:11434/api/chat";

/// Модель можно переопределить без перекомпиляции:
/// ARMADILLO_OLLAMA_MODEL=qwen2.5vl:3b
///
/// По умолчанию qwen3:4b-instruct (~2.5GB): чисто текстовая модель
/// (без неиспользуемого lookup'ом vision-блока VL-версий), поколение
/// новее и заметно сильнее в переводе на русский. Целиком живёт в
/// 6GB VRAM даже при типичном десктопе в фоне. Модели крупнее (7b+)
/// на этом GPU уходят в CPU-offload: ~12 tok/s, ответ за 9-17 секунд.
fn ollama_model() -> String {
    std::env::var("ARMADILLO_OLLAMA_MODEL").unwrap_or_else(|_| "qwen3:4b-instruct".to_string())
}

/// Полный список тем lookup — единственный источник истины: из него
/// строится и перечисление (enum) в JSON-схеме structured output, и
/// список тем в системном промпте, поэтому они не могут разойтись.
/// Эмодзи намеренно не входят: модель их не различает, фронту они
/// тоже не нужны.
const TOPICS: [&str; 30] = [
    "Дом и быт",
    "Семья и отношения",
    "Еда и напитки",
    "Одежда и аксессуары",
    "Покупки и магазины",
    "Путешествия",
    "Транспорт",
    "Город и места",
    "Работа и профессии",
    "Учёба и образование",
    "Внешность и характер",
    "Эмоции и чувства",
    "Время, даты и календарь",
    "Погода и природа",
    "Здоровье и тело",
    "Спорт и физическая активность",
    "Хобби и досуг",
    "Технологии и гаджеты",
    "Интернет и социальные сети",
    "Кино, музыка и развлечения",
    "Страны и национальности",
    "Деньги и финансы",
    "Ресторан и кафе",
    "Отель и гостиница",
    "Домашние дела и уборка",
    "Природа и животные",
    "Повседневные действия и глаголы",
    "Общение и социальные фразы",
    "Телефон, сообщения и переписка",
    "Любовь, знакомства и отношения",
];

/// Приводит topic из ответа модели к каноническому значению из TOPICS.
/// Схема (enum) уже не даёт модели выйти за список, но парсер не
/// доверяет ответу слепо: тема сверяется со списком (точное совпадение
/// или без учёта регистра) и канонизируется; неизвестное значение
/// возвращается как есть — деградация вместо падения.
fn canonicalize_topic(topic: &str) -> String {
    let trimmed = topic.trim();

    if trimmed.is_empty() {
        return String::new();
    }

    if TOPICS.contains(&trimmed) {
        return trimmed.to_string();
    }

    let lowered = trimmed.to_lowercase();

    TOPICS
        .iter()
        .find(|candidate| candidate.to_lowercase() == lowered)
        .map(|candidate| (*candidate).to_string())
        .unwrap_or_else(|| trimmed.to_string())
}

/// Системный промпт: правила поведения модели. Строится функцией,
/// чтобы список тем подставлялся из TOPICS и не дублировался вручную.
///
/// Разделение system/user важно для маленькой модели: «как отвечать»
/// живёт в system, а данные (предложение и слово) идут отдельным
/// user-сообщением.
///
/// Ключевые требования:
/// - sentence_translation — ПОЛНЫЙ русский перевод предложения, это
///   главный результат lookup;
/// - word_translation обязан встречаться в sentence_translation в той
///   же грамматической форме: фронтенд подсвечивает слово в переводе
///   по вхождению этой строки;
/// - meaning — одно простое английское предложение уровня A1-A2,
///   объясняющее значение слова, без самого слова и без перевода;
/// - synonyms — только реальные синонимы (взаимозаменяемы в этом же
///   предложении): пустой массив — валидный ответ, у большинства
///   конкретных существительных синонимов нет; тематические соседи
///   («крокодил»/«черепаха» для armadillo) — НЕ синонимы;
/// - транслитерация запрещена: «armadillo» → «броненосец»,
///   никогда «Армадилло» (слова «армадилло» в русском нет).
fn system_prompt() -> String {
    let topics = TOPICS.join(" / ");

    format!(
        r#"You are an English learning assistant. The user gives you an English sentence (extracted from the screen by OCR, so it may contain minor artifacts) and a target English word from it.

Reply with exactly one JSON object with these fields:
- "word": string. Copy the target word exactly as given.
- "sentence_translation": string. The complete, natural Russian translation of the WHOLE context sentence (or of the single word if the context is one word). Just the translation: no explanations, no alternatives, no transliteration.
- "word_translation": string. The natural Russian translation of the target word AS USED IN THIS CONTEXT, in the same grammatical form as it appears inside sentence_translation, so that this exact string occurs inside sentence_translation. Example: for "more" in "More instructions" it must be "Больше", not "более". NEVER transliterate the English word into Russian letters: the English word "armadillo" is "броненосец" in Russian, never "Армадилло" — such words do not exist in Russian.
- "meaning": string. ONE short simple English sentence at A1-A2 level, using only common words, that explains what the target word means in this context. Never use the target word itself inside meaning. Do not just repeat sentence_translation.
- "synonyms": array of English words that are TRUE synonyms of the target word in this context: they must be interchangeable with the target word in the SAME sentence with almost no change of meaning, and be the same part of speech. Words from the same category are NOT synonyms: for "armadillo" the words "crocodile" and "turtle" are different animals, not synonyms. If the word has no true synonyms — return an empty array []. Most concrete nouns (animals, objects, places) have no synonyms at all. Maximum 3 items. Never Russian words. Never the target word itself.
- "part_of_speech": string. Grammatical category of the word in this sentence (noun, verb, adjective, adverb, ...).
- "topic": string. EXACTLY one value from this list, nothing else: {topics}

Example reply for the word "run" in the sentence "He likes to run in the park every morning":
{{"word":"run","sentence_translation":"Он любит бегать в парке каждое утро.","word_translation":"бегать","meaning":"He moves fast using his legs.","synonyms":["jog","sprint"],"part_of_speech":"verb","topic":"Спорт и физическая активность"}}

Example reply for the word "armadillo" in "The armadillo rolled into a ball" (note the real Russian word, not a transliteration, and empty synonyms):
{{"word":"armadillo","sentence_translation":"Броненосец свернулся в клубок.","word_translation":"Броненосец","meaning":"It is a small animal with a hard shell on its back.","synonyms":[],"part_of_speech":"noun","topic":"Природа и животные"}}

Use the context to resolve ambiguity. Do not invent meanings unsupported by the context."#,
        topics = topics
    )
}

/// JSON Schema для structured output Ollama (поле "format").
///
/// В отличие от "format": "json", схема гарантирует не просто валидный
/// JSON, а объект со ВСЕМИ обязательными полями нужных типов: грамматика
/// генерации физически не позволяет закрыть объект без "synonyms" и
/// остальных полей. Это чинит реальный кейс qwen2.5vl:3b, когда модель
/// останавливалась после "meaning" и lookup падал с
/// "missing field `synonyms`".
fn lookup_format() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "word": { "type": "string" },
            "sentence_translation": { "type": "string" },
            "word_translation": { "type": "string" },
            "meaning": { "type": "string" },
            "synonyms": { "type": "array", "items": { "type": "string" } },
            "part_of_speech": { "type": "string" },
            // Enum из TOPICS: грамматика генерации не выпустит тему вне списка.
            "topic": { "type": "string", "enum": TOPICS }
        },
        "required": [
            "word",
            "sentence_translation",
            "word_translation",
            "meaning",
            "synonyms",
            "part_of_speech",
            "topic"
        ]
    })
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
    /// Всё обращение к API (запрос и ответ, включая ошибки) логируется
    /// в консоль — DEBUG-вывод для отладки.
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

        // DEBUG: что уходит в API.
        println!("=== >>> Ollama REQUEST http://localhost:11434/api/generate (warm-up) ===");
        println!(
            "{}",
            serde_json::to_string_pretty(&request).unwrap_or_else(|_| request.to_string())
        );
        println!("=== >>> END Ollama REQUEST ===\n");

        let result = self
            .client
            .post("http://localhost:11434/api/generate")
            .json(&request)
            .send();

        match result {
            Ok(response) => {
                let status = response.status();

                let response_text = response.text().unwrap_or_default();

                // DEBUG: что пришло из API.
                println!(
                    "=== <<< Ollama RESPONSE (HTTP {status}, {:.2} s) ===",
                    started.elapsed().as_secs_f64()
                );
                println!("{response_text}");
                println!("=== <<< END Ollama RESPONSE ===\n");

                if status.is_success() {
                    println!(
                        "AI model warm-up finished in {:.2} s",
                        started.elapsed().as_secs_f64()
                    );

                    true
                } else {
                    false
                }
            }
            Err(error) => {
                println!("Ollama warm-up request failed: {error}");

                false
            }
        }
    }

    /// Один запрос к /api/chat: отправляет сообщения, замеряет время
    /// и парсит ответ модели.
    ///
    /// Возвращает распознанный объект и сырой текст ответа — сырой текст
    /// нужен корректирующему повтору как сообщение ассистента.
    fn request_lookup(
        &self,
        messages: serde_json::Value,
    ) -> Result<(LocalLookup, String), String> {
        let request = json!({
            "model": ollama_model(),
            "messages": messages,
            "stream": false,
            "keep_alive": -1,
            // Грамматическое ограничение Ollama (structured output): ответ
            // обязан быть JSON-объектом ровно по схеме — модель не может
            // закрыть объект без обязательных полей, вставить prose или
            // markdown-заборы.
            "format": lookup_format(),
            "options": {
                "num_ctx": 2048,
                // Кириллица токенизируется заметно дороже английского
                // (~2-3 токена на слово), а обрыв по лимиту делает JSON
                // невалидным. 512 покрывает полный ответ с запасом.
                "num_predict": 512,
                "temperature": 0.1
            }
        });

        // DEBUG: что уходит в API.
        println!("=== >>> Ollama REQUEST {OLLAMA_URL} ===");
        println!(
            "{}",
            serde_json::to_string_pretty(&request).unwrap_or_else(|_| request.to_string())
        );
        println!("=== >>> END Ollama REQUEST ===\n");

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

        // DEBUG: что пришло из API.
        println!("=== <<< Ollama RESPONSE (HTTP {status}) ===");
        println!("{response_text}");
        println!("=== <<< END Ollama RESPONSE ===\n");

        if !status.is_success() {
            return Err(format!("Ollama returned HTTP {status}: {response_text}"));
        }

        let response: LocalResponse = serde_json::from_str(&response_text).map_err(|error| {
            format!("Failed to parse Ollama API response: {error}\nResponse: {response_text}")
        })?;

        let content = strip_json_fences(&response.message.content.unwrap_or_default());

        let parsed: LocalLookup = serde_json::from_str(&content).map_err(|error| {
            format!("Failed to parse lookup JSON: {error}\nContent: {content}")
        })?;

        Ok((parsed, content))
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
    #[serde(default)]
    meaning: String,
    #[serde(default)]
    sentence_translation: String,
    #[serde(default)]
    word_translation: String,
    /// Модель порой шлёт строку вместо массива или вовсе пропускает поле.
    /// Принимаем любой JSON-тип и нормализуем, чтобы парсер не падал.
    #[serde(default)]
    synonyms: serde_json::Value,
    #[serde(default)]
    part_of_speech: String,
    /// Модель порой возвращает массив вопреки промпту. Принимаем
    /// любой JSON-тип и нормализуем в строку, чтобы парсер не падал.
    #[serde(default)]
    topic: serde_json::Value,
}

/// Проверка «текст на русском»: хотя бы один символ кириллицы.
fn contains_cyrillic(text: &str) -> bool {
    text.chars()
        .any(|character| ('\u{0400}'..='\u{04FF}').contains(&character))
}

/// Приводит topic к строке: модель иногда шлёт массив
/// ["программирование", "код"] вместо строки.
fn topic_to_string(topic: &serde_json::Value) -> String {
    match topic {
        serde_json::Value::String(text) => text.trim().to_string(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| item.as_str())
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        serde_json::Value::Null => String::new(),
        other => other.to_string().trim_matches('"').to_string(),
    }
}

/// Приводит synonyms к массиву строк: модель иногда шлёт одну строку
/// («jog, sprint») вместо массива, а при serde(default) поле может
/// отсутствовать вовсе.
fn synonyms_to_vec(synonyms: &serde_json::Value) -> Vec<String> {
    match synonyms {
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| item.as_str())
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
            .collect(),
        serde_json::Value::String(text) => text
            .split(',')
            .map(str::trim)
            .filter(|word| !word.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

/// Грубая транслитерация кириллицы в латиницу. Не для вывода, а для
/// детекции одного систематического сбоя модели: вместо перевода она
/// пишет английское слово русскими буквами («Армадилло» для
/// "armadillo") — а такого слова в русском нет.
fn translit_cyrillic_to_latin(text: &str) -> String {
    let mut out = String::with_capacity(text.len());

    for character in text.to_lowercase().chars() {
        let replacement = match character {
            'а' => "a",
            'б' => "b",
            'в' => "v",
            'г' => "g",
            'д' => "d",
            'е' | 'ё' => "e",
            'ж' => "zh",
            'з' => "z",
            'и' => "i",
            'й' => "y",
            'к' => "k",
            'л' => "l",
            'м' => "m",
            'н' => "n",
            'о' => "o",
            'п' => "p",
            'р' => "r",
            'с' => "s",
            'т' => "t",
            'у' => "u",
            'ф' => "f",
            'х' => "h",
            'ц' => "ts",
            'ч' => "ch",
            'ш' => "sh",
            'щ' => "shch",
            'ъ' | 'ь' => "",
            'ы' => "y",
            'э' => "e",
            'ю' => "yu",
            'я' => "ya",
            other => {
                out.push(other);

                continue;
            }
        };

        out.push_str(replacement);
    }

    out
}

/// Детекция транслитерации: в русском тексте (word_translation или
/// sentence_translation) стоит слово, которое в обратной транслитерации
/// совпадает с английским оригиналом. «Армадилло» → "armadillo" — да,
/// «Броненосец» → "brononosets" — нет.
///
/// Точность достаточная: цель не лингвистический анализ, а ловля
/// конкретного сбоя. Возможное ложное срабатывание на легитимном
/// заимствовании («стоп» для "stop") стоит одного лишнего повтора,
/// не корректности: повтор вернёт то же слово, и оно будет показано.
fn is_transliteration(word: &str, russian_text: &str) -> bool {
    let target = word
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase();

    if target.is_empty() {
        return false;
    }

    russian_text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| {
            token
                .chars()
                .any(|character| ('\u{0400}'..='\u{04FF}').contains(&character))
        })
        .any(|token| {
            let back = translit_cyrillic_to_latin(token);

            !back.is_empty() && (back == target || format!("{back}s") == target)
        })
}

/// Детерминированная чистка синонимов: модель подсовывает само целевое
/// слово, дубликат в другом регистре или пустые строки. Лимит 3 —
/// синонимы в UI вторичны.
fn clean_synonyms(word: &str, synonyms: Vec<String>) -> Vec<String> {
    let target = word.trim().to_lowercase();

    let mut cleaned: Vec<String> = Vec::new();

    for synonym in synonyms {
        let trimmed = synonym.trim();

        if trimmed.is_empty() {
            continue;
        }

        let lowered = trimmed.to_lowercase();

        if lowered == target
            || cleaned
                .iter()
                .any(|existing| existing.to_lowercase() == lowered)
        {
            continue;
        }

        cleaned.push(trimmed.to_string());

        if cleaned.len() >= 3 {
            break;
        }
    }

    cleaned
}

/// Фронтенд подсвечивает перевод слова внутри перевода предложения
/// точным вхождением строки. Промпт требует вернуть слово в той же
/// грамматической форме, но маленькая модель иногда ошибается в форме
/// («кэширование» вместо стоящего в переводе «кэширования») или в
/// регистре (слово в начале предложения с заглавной буквы). Тогда
/// ищем реальную форму слова в переводе и подставляем её — иначе
/// подсветка во фронтенде не сработает.
///
/// Стратегия: точное вхождение → регистронезависимое вхождение →
/// совпадение по стему (слово без двух последних букв — окончание).
/// Если ничего не нашлось, возвращаем как есть: текст осмысленный,
/// просто без подсветки.
fn align_word_translation(word_translation: &str, sentence_translation: &str) -> String {
    let word_translation = word_translation.trim();

    if word_translation.is_empty() {
        return String::new();
    }

    // 1. Точное вхождение — лучший случай, ничего делать не нужно.
    if sentence_translation.contains(word_translation) {
        return word_translation.to_string();
    }

    let needle: Vec<char> = word_translation.to_lowercase().chars().collect();

    let sentence: Vec<char> = sentence_translation.chars().collect();

    // 2. Регистронезависимое вхождение («сжатие» vs «Сжатие»):
    // возвращаем форму в том виде, как она стоит в предложении,
    // чтобы точный contains() на фронтенде совпал.
    if needle.len() <= sentence.len() {
        for start in 0..=(sentence.len() - needle.len()) {
            let matches = sentence[start..start + needle.len()]
                .iter()
                .zip(needle.iter())
                .all(|(from_sentence, from_word)| {
                    from_sentence
                        .to_lowercase()
                        .eq(from_word.to_lowercase())
                });

            if matches {
                let found: String = sentence[start..start + needle.len()].iter().collect();

                println!(
                    "word_translation '{word_translation}' aligned to '{found}' from sentence translation"
                );

                return found;
            }
        }
    }

    // 3. Совпадение по основе слова: модель вернула другую форму того же
    // слова (другой падеж, причастие vs глагол). Считаем слова однокоренными,
    // если их общий префикс покрывает бо́льшую часть обеих форм, и возвращаем
    // форму, реально стоящую в переводе, — точный contains() на фронтенде
    // совпадёт. Порог в 55% и минимум 5 символов отсекают случайные
    // совпадения первых букв («сжатие»/«сжимаются» не сматчатся намеренно:
    // их общий префикс «сж» короче минимума).
    let needle_lower: String = needle.iter().collect::<String>().to_lowercase();

    let needle_len = needle_lower.chars().count();

    let best = sentence_translation
        .split(|character: char| !character.is_alphanumeric() && character != '-')
        .filter(|candidate| !candidate.is_empty())
        .map(|candidate| {
            let candidate_lower = candidate.to_lowercase();

            let common = needle_lower
                .chars()
                .zip(candidate_lower.chars())
                .take_while(|(from_needle, from_candidate)| from_needle == from_candidate)
                .count();

            (common, candidate)
        })
        .filter(|(common, candidate)| {
            let candidate_len = candidate.to_lowercase().chars().count();

            // 55% покрытия обеих форм (common * 20 >= len * 11) и минимум
            // 5 совпавших символов. Не 60%: реальный кейс «компрессир» (10)
            // от «компрессированный» (17) — это 58.8%, и слово однокоренное.
            *common >= 5 && common * 20 >= needle_len * 11 && common * 20 >= candidate_len * 11
        })
        .max_by_key(|(common, _)| *common);

    match best {
        Some((_, found)) => {
            println!(
                "word_translation '{word_translation}' aligned to '{found}' from sentence translation"
            );

            found.to_string()
        }
        None => word_translation.to_string(),
    }
}

/// Маленькие модели иногда оборачивают JSON в markdown-забор ```json ... ```
/// или добавляют текст до/после объекта. Срезаем всё до первого `{`
/// и после последнего `}`, чтобы парсер получил чистый объект.
fn strip_json_fences(text: &str) -> String {
    let trimmed = text.trim();

    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start <= end {
            return trimmed[start..=end].to_string();
        }
    }

    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // align_word_translation: подсветка слова во фронтенде держится на
    // точном вхождении строки в перевод предложения, поэтому все три
    // ступени выравнивания критичны.
    // ------------------------------------------------------------------

    #[test]
    fn align_returns_exact_form_when_already_present() {
        let sentence = "Данные эффективно уменьшаются за счёт сжатия перед записью.";

        assert_eq!(
            align_word_translation("сжатия", sentence),
            "сжатия",
            "точное вхождение должно возвращаться как есть"
        );
    }

    #[test]
    fn align_fixes_case_mismatch() {
        // Слово стоит в начале предложения: модель вернула строчную форму,
        // а в переводе оно с заглавной. Подсветка без выравнивания сломалась бы.
        let sentence = "Сжатие уменьшает объём данных.";

        assert_eq!(
            align_word_translation("сжатие", sentence),
            "Сжатие",
            "регистронезависимое вхождение должно вернуть форму из перевода"
        );
    }

    #[test]
    fn align_fixes_grammatical_form_via_stem() {
        // Модель вернула именительный падеж, а в переводе другой падеж.
        let sentence = "Это ускоряет кэширование страниц.";

        assert_eq!(
            align_word_translation("кэширования", sentence),
            "кэширование",
            "стем-совпадение должно вернуть реальную форму из перевода"
        );
    }

    #[test]
    fn align_fixes_derived_form_via_common_prefix() {
        // Реальный кейс с qwen2.5vl:3b: модель вернула «компрессированный»,
        // в переводе стоит «компрессируются». Общий префикс «компрессир» (10)
        // покрывает 58.8% длинной формы — порог 55% пропускает её как
        // однокоренную (60% отверг бы этот настоящий кейс).
        let sentence = "Файлы компрессируются перед их загрузкой на сервер.";

        assert_eq!(
            align_word_translation("компрессированный", sentence),
            "компрессируются",
            "совпадение по общему префиксу должно вернуть форму из перевода"
        );
    }

    #[test]
    fn align_rejects_short_common_prefix() {
        // «сжатие» и «сжимаются» — разные основы: общий префикс «сж» короче
        // порога. Ложное совпадение здесь дал бы неправильную подсветку.
        let sentence = "Файлы сжимаются перед записью.";

        assert_eq!(
            align_word_translation("сжатие", sentence),
            "сжатие",
            "короткий общий префикс не должен считаться однокоренностью"
        );
    }

    #[test]
    fn align_falls_back_to_model_output_when_nothing_matches() {
        let sentence = "Полностью не связанный текст.";

        assert_eq!(
            align_word_translation("сжатие", sentence),
            "сжатие",
            "без совпадений возвращаем ответ модели как есть"
        );
    }

    #[test]
    fn align_handles_empty_word_translation() {
        assert_eq!(align_word_translation("   ", "Любой перевод."), "");
    }

    // ------------------------------------------------------------------
    // Нормализация ответов маленькой модели.
    // ------------------------------------------------------------------

    // ------------------------------------------------------------------
    // Темы: единый список для промпта и схемы + канонизация.
    // ------------------------------------------------------------------

    #[test]
    fn topics_are_unique_and_non_empty() {
        assert_eq!(TOPICS.len(), 30);

        let mut seen = std::collections::HashSet::new();

        for topic in TOPICS {
            assert!(!topic.trim().is_empty(), "пустая тема в списке");
            assert!(seen.insert(topic), "тема {topic:?} повторяется в списке");
        }
    }

    #[test]
    fn lookup_format_topic_is_enum_of_topics() {
        let format = lookup_format();

        let variants = format["properties"]["topic"]["enum"]
            .as_array()
            .expect("topic в схеме должен быть перечислением");

        assert_eq!(variants.len(), TOPICS.len());

        for (variant, topic) in variants.iter().zip(TOPICS) {
            assert_eq!(variant.as_str(), Some(topic));
        }
    }

    #[test]
    fn system_prompt_lists_every_topic() {
        let prompt = system_prompt();

        for topic in TOPICS {
            assert!(prompt.contains(topic), "промпт должен называть тему {topic:?}");
        }
    }

    #[test]
    fn canonicalize_topic_matches_known_values() {
        assert_eq!(
            canonicalize_topic("Спорт и физическая активность"),
            "Спорт и физическая активность"
        );

        // Регистр и пробелы по краям не важны.
        assert_eq!(
            canonicalize_topic("  спорт и физическая активность "),
            "Спорт и физическая активность"
        );
    }

    #[test]
    fn canonicalize_topic_keeps_unknown_as_is() {
        // Модель без поддержки schema может прислать что угодно:
        // деградация (показать как есть) вместо падения lookup.
        assert_eq!(canonicalize_topic("программирование"), "программирование");
        assert_eq!(canonicalize_topic("   "), "");
    }

    #[test]
    #[ignore = "требует запущенного Ollama: реальный вызов модели"]
    fn real_lookup_smoke() {
        let provider = LocalProvider::new().expect("провайдер должен создаваться");

        let started = Instant::now();

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

        assert!(
            contains_cyrillic(&result.sentence_translation),
            "перевод предложения должен быть на русском"
        );
    }

    #[test]
    #[ignore = "требует запущенного Ollama: реальный вызов модели"]
    fn real_lookup_armadillo() {
        let provider = LocalProvider::new().expect("провайдер должен создаваться");

        let started = Instant::now();

        let result = provider
            .lookup("The armadillo rolled into a ball.", "armadillo")
            .expect("lookup должен отработать");

        println!("lookup took {:.2} s", started.elapsed().as_secs_f64());
        println!("sentence_translation: {}", result.sentence_translation);
        println!("word_translation: {}", result.word_translation);
        println!("meaning: {}", result.meaning);
        println!("synonyms: {:?}", result.synonyms);

        let sentence = result.sentence_translation.to_lowercase();

        assert!(
            sentence.contains("броненосец"),
            "должно быть настоящее русское слово, а не транслитерация: {}",
            result.sentence_translation
        );
        assert!(
            !sentence.contains("армадилло"),
            "транслитерация просочилась в перевод: {}",
            result.sentence_translation
        );

        let forbidden = ["crocodile", "turtle", "крокодил", "черепах"];

        for synonym in &result.synonyms {
            let lowered = synonym.to_lowercase();

            assert!(
                !forbidden.iter().any(|bad| lowered.contains(bad)),
                "тематический сосед вместо синонима: {synonym}"
            );
        }
    }

    #[test]
    fn topic_string_passes_through() {
        assert_eq!(
            topic_to_string(&serde_json::Value::String(" программирование ".into())),
            "программирование"
        );
    }

    #[test]
    fn topic_array_is_joined() {
        assert_eq!(
            topic_to_string(&serde_json::json!(["программирование", "код"])),
            "программирование, код"
        );
    }

    #[test]
    fn topic_null_is_empty() {
        assert_eq!(topic_to_string(&serde_json::Value::Null), "");
    }

    #[test]
    fn synonyms_array_passes_through() {
        assert_eq!(
            synonyms_to_vec(&serde_json::json!(["jog", " sprint "])),
            vec!["jog", "sprint"]
        );
    }

    #[test]
    fn synonyms_string_is_split() {
        assert_eq!(
            synonyms_to_vec(&serde_json::Value::String("jog, sprint".into())),
            vec!["jog", "sprint"]
        );
    }

    #[test]
    fn synonyms_missing_or_wrong_type_is_empty() {
        assert!(synonyms_to_vec(&serde_json::Value::Null).is_empty());
        assert!(synonyms_to_vec(&serde_json::Value::Bool(true)).is_empty());
    }

    // ------------------------------------------------------------------
    // Транслитерация и чистка синонимов: модель периодически пишет
    // «Армадилло» вместо «броненосец» и подсовывает тематических
    // соседей вместо синонимов.
    // ------------------------------------------------------------------

    #[test]
    fn translit_detects_transliterated_word() {
        assert!(is_transliteration("armadillo", "Армадилло"));
        assert!(is_transliteration(
            "armadillo",
            "Армадилло свернулся в клубок."
        ));
    }

    #[test]
    fn translit_ignores_real_translations() {
        assert!(!is_transliteration("armadillo", "Броненосец"));
        assert!(!is_transliteration(
            "armadillo",
            "Броненосец свернулся в клубок."
        ));
        assert!(!is_transliteration("compression", "Сжатие данных."));
    }

    #[test]
    fn clean_synonyms_drops_target_and_duplicates() {
        assert_eq!(
            clean_synonyms(
                "run",
                vec![
                    "Run".to_string(),
                    "jog".to_string(),
                    "  jog  ".to_string(),
                    String::new(),
                    "sprint".to_string(),
                ]
            ),
            vec!["jog".to_string(), "sprint".to_string()]
        );
    }

    #[test]
    fn clean_synonyms_caps_at_three() {
        let synonyms = clean_synonyms(
            "fast",
            vec!["quick", "swift", "rapid", "speedy", "hasty"]
                .into_iter()
                .map(String::from)
                .collect(),
        );

        assert_eq!(synonyms.len(), 3);
        assert_eq!(synonyms, vec!["quick", "swift", "rapid"]);
    }

    #[test]
    fn prompt_bans_transliteration_and_allows_empty_synonyms() {
        let prompt = system_prompt();

        assert!(
            prompt.to_lowercase().contains("never transliterate"),
            "промпт должен запрещать транслитерацию"
        );
        assert!(
            prompt.contains("Броненосец") && prompt.contains("armadillo"),
            "промпт должен содержать пример armadillo → броненосец"
        );
        assert!(
            prompt.contains("empty array"),
            "промпт должен разрешать пустой список синонимов"
        );
        assert!(
            prompt.contains("NOT synonyms"),
            "промпт должен объяснять, что тематические соседи — не синонимы"
        );
    }

    #[test]
    fn lookup_parses_response_without_optional_fields() {
        // Реальный кейс qwen2.5vl:3b: модель оборвала ответ после "meaning",
        // и lookup падал с "missing field `synonyms`". Теперь такой ответ
        // парсится и деградирует до пустых значений.
        let content = r#"{"word":"Otherwise","sentence_translation":"Иначе вы просто уйдете.","word_translation":"Иначе","meaning":"Иначе означает "}"#;

        let parsed: LocalLookup =
            serde_json::from_str(content).expect("ответ без части полей должен парситься");

        assert_eq!(parsed.word_translation, "Иначе");
        assert_eq!(parsed.sentence_translation, "Иначе вы просто уйдете.");
        assert!(synonyms_to_vec(&parsed.synonyms).is_empty());
        assert_eq!(parsed.part_of_speech, "");
        assert_eq!(topic_to_string(&parsed.topic), "");
    }

    #[test]
    fn lookup_format_requires_all_fields() {
        let format = lookup_format();

        let required = format["required"]
            .as_array()
            .expect("required должен быть массивом");

        for field in [
            "word",
            "sentence_translation",
            "word_translation",
            "meaning",
            "synonyms",
            "part_of_speech",
            "topic",
        ] {
            assert!(
                required.iter().any(|value| value.as_str() == Some(field)),
                "поле {field} должно быть обязательным"
            );
        }

        assert_eq!(format["properties"]["synonyms"]["type"], "array");
    }

    #[test]
    fn cyrillic_detection() {
        assert!(contains_cyrillic("Сжатие данных"));
        assert!(!contains_cyrillic("plain compression"));
        assert!(!contains_cyrillic(""));
    }

    #[test]
    fn fences_are_stripped() {
        assert_eq!(
            strip_json_fences("```json\n{\"a\":1}\n```"),
            "{\"a\":1}"
        );

        assert_eq!(strip_json_fences("  {\"a\":1}  "), "{\"a\":1}");
    }
}

impl AiProvider for LocalProvider {
    fn lookup(&self, sentence: &str, word: &str) -> Result<LookupResult, String> {
        // DEBUG: краткая сводка о том, что ищем (полный запрос печатает request_lookup).
        println!("[lookup] word=\"{word}\", sentence=\"{sentence}\"");

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

        // Гарантия «перевод приходит на русском»: маленькая модель
        // иногда игнорирует язык и возвращает копию английского текста
        // или пустую строку. Ловим это по отсутствию кириллицы и делаем
        // один корректирующий повтор с явным указанием на ошибку.
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

        // Детекция транслитерации: модель иногда пишет английское слово
        // русскими буквами («Армадилло») вместо настоящего русского
        // слова («броненосец»). Промпт это запрещает, но модель нарушает
        // запрет; один корректирующий повтор с явным разбором ошибки
        // чинит. Ровно один повтор, не цикл: если модель транслитерирует
        // снова — показываем как есть (деградация вместо задержек).
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
