use std::fs;
use std::sync::Arc;
use std::time::{ Duration, SystemTime, UNIX_EPOCH };

use base64::{ engine::general_purpose, Engine as _ };
use reqwest::blocking::Client;
use serde::Deserialize;

use screenshots::Screen;

use crate::app_core::input::event::InputEvent;
use crate::app_core::overlay::manager::OverlayManager;

const GROQ_URL: &str = "https://api.groq.com/openai/v1/chat/completions";
const GROQ_MODEL: &str = "qwen/qwen3.6-27b";

// Размер области вокруг точки, которую отправляем vision-модели.
const CROP_WIDTH: u32 = 800;
const CROP_HEIGHT: u32 = 600;

// Увеличиваем crop перед отправкой.
const CROP_SCALE: u32 = 2;

// =========================================================
// LOOKUP RESULT
// =========================================================

#[derive(Debug, Deserialize)]
pub struct LookupResult {
    pub sentence: String,
    pub word: String,
    pub sentence_translation: String,
    pub word_translation: String,
    pub synonyms: Vec<String>,
    pub part_of_speech: String,
    pub topic: String,
}

// =========================================================
// GROQ RESPONSE
// =========================================================

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

// =========================================================
// PIPELINE
// =========================================================

pub struct ClickPipeline {
    overlay: Arc<OverlayManager>,
    client: Client,
}

impl ClickPipeline {
    pub fn new(overlay: Arc<OverlayManager>) -> Result<Self, String> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|error| { format!("Failed to create HTTP client: {error}") })?;

        Ok(Self {
            overlay,
            client,
        })
    }

    // =========================================================
    // SCREEN CAPTURE
    // =========================================================

    fn capture_screen() -> Result<Image, String> {
        println!("Getting screens...");

        let screens = Screen::all().map_err(|error| { format!("Failed to get screens: {error}") })?;

        let screen = screens.first().ok_or_else(|| "No screens found".to_string())?;

        println!("Capturing first screen...");

        let screenshot = screen
            .capture()
            .map_err(|error| { format!("Failed to capture screen: {error}") })?;

        let width = screenshot.width();
        let height = screenshot.height();

        println!("Screenshot captured: {}x{}", width, height);

        let pixels = screenshot.as_raw();

        // screenshots crate отдаёт BGRA:
        //
        // B G R A
        // B G R A
        // ...

        let expected_len = (width as usize) * (height as usize) * 4;

        if pixels.len() != expected_len {
            return Err(
                format!("Invalid frame size: expected {}, got {}", expected_len, pixels.len())
            );
        }

        // BGRA -> RGB.
        let mut rgb = Vec::with_capacity((width as usize) * (height as usize) * 3);

        for pixel in pixels.chunks_exact(4) {
            let b = pixel[0];
            let g = pixel[1];
            let r = pixel[2];

            rgb.push(r);
            rgb.push(g);
            rgb.push(b);
        }

        Ok(Image {
            width,
            height,
            data: rgb,
        })
    }

    // =========================================================
    // PROCESS INPUT EVENT
    // =========================================================

    pub fn process(&self, event: InputEvent) {
        match event {
            InputEvent::Lookup { x, y } => {
                println!("ClickPipeline: Lookup at ({x}, {y})");

                match self.lookup(x, y) {
                    Ok(result) => {
                        println!("=== LOOKUP RESULT ===");

                        println!("sentence: {}", result.sentence);

                        println!("word: {}", result.word);

                        println!("sentence_translation: {}", result.sentence_translation);

                        println!("word_translation: {}", result.word_translation);

                        println!("synonyms: {:?}", result.synonyms);

                        println!("part_of_speech: {}", result.part_of_speech);

                        println!("topic: {}", result.topic);

                        println!("=== END LOOKUP RESULT ===");

                        self.overlay.show(x, y);
                    }

                    Err(error) => {
                        eprintln!("Lookup failed: {error}");
                    }
                }
            }
        }
    }

    // =========================================================
    // LOOKUP
    // =========================================================

    fn lookup(&self, click_x: i32, click_y: i32) -> Result<LookupResult, String> {
        println!("=== LOOKUP START ===");

        // -----------------------------------------------------
        // API KEY
        // -----------------------------------------------------

        let api_key = std::env
            ::var("GROQ_API_KEY")
            .map_err(|_| { "GROQ_API_KEY environment variable is not set".to_string() })?;

        // -----------------------------------------------------
        // SCREENSHOT
        // -----------------------------------------------------

        println!("Capturing full screen...");

        let full_image = Self::capture_screen()?;

        println!("Captured: {}x{}", full_image.width, full_image.height);

        // -----------------------------------------------------
        // CLICK MARKER
        // -----------------------------------------------------

        let mut marked_image = full_image;

        draw_click_marker(&mut marked_image, click_x, click_y);

        println!("Click marker drawn at ({}, {})", click_x, click_y);

        // -----------------------------------------------------
        // CROP AROUND CLICK
        // -----------------------------------------------------

        let cropped = crop_around_point(&marked_image, click_x, click_y, CROP_WIDTH, CROP_HEIGHT);

        println!("Vision crop: {}x{}", cropped.width, cropped.height);

        // -----------------------------------------------------
        // UPSCALE
        // -----------------------------------------------------

        let vision_image = upscale_nearest(&cropped, CROP_SCALE);

        println!("Vision image: {}x{}", vision_image.width, vision_image.height);

        // -----------------------------------------------------
        // PNG
        // -----------------------------------------------------

        let png = encode_png(&vision_image)?;

        println!("PNG size: {} bytes", png.len());

        // -----------------------------------------------------
        // DEBUG SCREENSHOT
        // -----------------------------------------------------

        save_debug_screenshot(&png, click_x, click_y)?;

        // -----------------------------------------------------
        // BASE64
        // -----------------------------------------------------

        let encoded = general_purpose::STANDARD.encode(&png);

        let image_url = format!("data:image/png;base64,{encoded}");

        // -----------------------------------------------------
        // SYSTEM PROMPT
        // -----------------------------------------------------

        let system_prompt =
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
- Do not use words from this instruction as the answer.
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

Field meanings:

sentence:
The sentence, code line, terminal line, or UI text
containing the selected word.

word:
The exact English word or short phrase physically located
under the CENTER of the yellow marker.

sentence_translation:
Natural Russian translation or explanation of the
sentence/code/UI text.

word_translation:
Natural Russian translation of the selected word.

synonyms:
Useful English synonyms.
Return [] when synonyms are not appropriate.

part_of_speech:
The grammatical part of speech of the selected word.
For programming identifiers, use the underlying English
part of speech when appropriate.

topic:
A short context description such as:
"programming",
"terminal",
"software development",
"IDE interface",
"business",
"travel",
"everyday English".

Return JSON only.
Do not use markdown.
Do not use ```json.
Do not include explanations outside the JSON object.
"#;

        // -----------------------------------------------------
        // USER MESSAGE
        // -----------------------------------------------------

        // Координаты сюда НЕ попадают.
        //
        // API получает только изображение с желтым маркером.

        let user_text =
            "Translate the English word or short phrase directly under the center of the yellow marker.";

        // -----------------------------------------------------
        // REQUEST
        // -----------------------------------------------------

        let request =
            serde_json::json!({
    "model": GROQ_MODEL,

    "messages": [
        {
            "role": "system",
            "content": system_prompt
        },
        {
            "role": "user",
            "content": [
                {
                    "type": "text",
                    "text": user_text
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

        // -----------------------------------------------------
        // SEND
        // -----------------------------------------------------

        println!("Sending screenshot to Groq...");

        let response = self.client
            .post(GROQ_URL)
            .bearer_auth(api_key)
            .json(&request)
            .send()
            .map_err(|error| { format!("Groq request failed: {error}") })?;

        let status = response.status();

        println!("Groq HTTP status: {}", status);

        let response_text = response
            .text()
            .map_err(|error| { format!("Failed to read Groq response: {error}") })?;

        if !status.is_success() {
            return Err(format!("Groq API returned {}: {}", status, response_text));
        }

        // -----------------------------------------------------
        // DEBUG RESPONSE
        // -----------------------------------------------------

        println!("=== GROQ API RESPONSE ===");
        println!("{response_text}");
        println!("=== END GROQ API RESPONSE ===");

        // -----------------------------------------------------
        // PARSE GROQ RESPONSE
        // -----------------------------------------------------

        let groq_response: GroqResponse = serde_json
            ::from_str(&response_text)
            .map_err(|error| { format!("Failed to parse Groq API response: {error}") })?;

        let content = groq_response.choices
            .first()
            .and_then(|choice| { choice.message.content.as_ref() })
            .ok_or_else(|| { "Groq response contains no message content".to_string() })?;

        println!("=== GROQ CONTENT ===");
        println!("{content}");
        println!("=== END GROQ CONTENT ===");

        // -----------------------------------------------------
        // PARSE LOOKUP RESULT
        // -----------------------------------------------------

        let result: LookupResult = serde_json
            ::from_str(content)
            .map_err(|error| {
                format!("Failed to parse lookup JSON: {}\nContent: {}", error, content)
            })?;

        println!("=== LOOKUP SUCCESS ===");

        Ok(result)
    }
}

// =========================================================
// IMAGE
// =========================================================

struct Image {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

// =========================================================
// CROP AROUND CLICK
// =========================================================

fn crop_around_point(
    source: &Image,
    click_x: i32,
    click_y: i32,
    crop_width: u32,
    crop_height: u32
) -> Image {
    let mut result = Image {
        width: crop_width,
        height: crop_height,
        data: vec![
            0;
            (crop_width as usize)
                * (crop_height as usize)
                * 3
        ],
    };

    let source_width = source.width as i32;
    let source_height = source.height as i32;

    let crop_width_i32 = crop_width as i32;
    let crop_height_i32 = crop_height as i32;

    // Точка клика находится в центре crop.

    let source_left = click_x - crop_width_i32 / 2;

    let source_top = click_y - crop_height_i32 / 2;

    for dst_y in 0..crop_height_i32 {
        for dst_x in 0..crop_width_i32 {
            let src_x = source_left + dst_x;

            let src_y = source_top + dst_y;

            if src_x < 0 || src_y < 0 || src_x >= source_width || src_y >= source_height {
                continue;
            }

            let src_index = ((src_y as usize) * (source.width as usize) + (src_x as usize)) * 3;

            let dst_index = ((dst_y as usize) * (crop_width as usize) + (dst_x as usize)) * 3;

            result.data[dst_index] = source.data[src_index];

            result.data[dst_index + 1] = source.data[src_index + 1];

            result.data[dst_index + 2] = source.data[src_index + 2];
        }
    }

    result
}

// =========================================================
// UPSCALE
// =========================================================

fn upscale_nearest(source: &Image, scale: u32) -> Image {
    if scale <= 1 {
        return Image {
            width: source.width,
            height: source.height,
            data: source.data.clone(),
        };
    }

    let width = source.width.saturating_mul(scale);

    let height = source.height.saturating_mul(scale);

    let mut data =
        vec![
        0;
        (width as usize)
            * (height as usize)
            * 3
    ];

    for y in 0..height {
        let source_y = y / scale;

        for x in 0..width {
            let source_x = x / scale;

            let source_index =
                ((source_y as usize) * (source.width as usize) + (source_x as usize)) * 3;

            let destination_index = ((y as usize) * (width as usize) + (x as usize)) * 3;

            data[destination_index] = source.data[source_index];

            data[destination_index + 1] = source.data[source_index + 1];

            data[destination_index + 2] = source.data[source_index + 2];
        }
    }

    Image {
        width,
        height,
        data,
    }
}

// =========================================================
// CLICK MARKER
// =========================================================

fn draw_click_marker(image: &mut Image, x: i32, y: i32) {
    const RADIUS: i32 = 14;

    const YELLOW_R: f32 = 255.0;
    const YELLOW_G: f32 = 220.0;
    const YELLOW_B: f32 = 0.0;

    const ALPHA: f32 = 0.4;

    let width = image.width as i32;
    let height = image.height as i32;

    // -----------------------------------------------------
    // TRANSPARENT CIRCLE
    // -----------------------------------------------------

    for dy in -RADIUS..=RADIUS {
        for dx in -RADIUS..=RADIUS {
            if dx * dx + dy * dy > RADIUS * RADIUS {
                continue;
            }

            let px = x + dx;
            let py = y + dy;

            if px < 0 || py < 0 || px >= width || py >= height {
                continue;
            }

            let index = ((py as usize) * (image.width as usize) + (px as usize)) * 3;

            if index + 2 >= image.data.len() {
                continue;
            }

            let r = image.data[index] as f32;

            let g = image.data[index + 1] as f32;

            let b = image.data[index + 2] as f32;

            image.data[index] = (r * (1.0 - ALPHA) + YELLOW_R * ALPHA).round() as u8;

            image.data[index + 1] = (g * (1.0 - ALPHA) + YELLOW_G * ALPHA).round() as u8;

            image.data[index + 2] = (b * (1.0 - ALPHA) + YELLOW_B * ALPHA).round() as u8;
        }
    }

    // -----------------------------------------------------
    // CENTER CROSS
    // -----------------------------------------------------

    for offset in -3..=3 {
        set_pixel(image, x + offset, y, 255, 220, 0);

        set_pixel(image, x, y + offset, 255, 220, 0);
    }
}

// =========================================================
// PIXEL
// =========================================================

fn set_pixel(image: &mut Image, x: i32, y: i32, r: u8, g: u8, b: u8) {
    if x < 0 || y < 0 || x >= (image.width as i32) || y >= (image.height as i32) {
        return;
    }

    let index = ((y as usize) * (image.width as usize) + (x as usize)) * 3;

    if index + 2 >= image.data.len() {
        return;
    }

    image.data[index] = r;
    image.data[index + 1] = g;
    image.data[index + 2] = b;
}

// =========================================================
// PNG
// =========================================================

fn encode_png(image: &Image) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;

    let mut output = Vec::<u8>::new();

    let encoder = image::codecs::png::PngEncoder::new(&mut output);

    encoder
        .write_image(&image.data, image.width, image.height, image::ExtendedColorType::Rgb8)
        .map_err(|error| { format!("Failed to encode PNG: {error}") })?;

    Ok(output)
}

// =========================================================
// DEBUG SCREENSHOT
// =========================================================
//
// ВАЖНО:
//
// Не сохраняем в src-tauri.
//
// Tauri dev watcher следит за src-tauri и при появлении
// PNG там перезапускает приложение.
//
// Поэтому debug screenshots идут в системный TEMP.
// =========================================================

fn save_debug_screenshot(png: &[u8], click_x: i32, click_y: i32) -> Result<(), String> {
    let directory = std::env::temp_dir().join("armadillo-screenshots");

    fs
        ::create_dir_all(&directory)
        .map_err(|error| {
            format!("Failed to create screenshots directory '{}': {}", directory.display(), error)
        })?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| { format!("Failed to get current timestamp: {error}") })?
        .as_millis();

    let path = directory.join(format!("lookup_{}_{}_{}.png", timestamp, click_x, click_y));

    fs
        ::write(&path, png)
        .map_err(|error| {
            format!("Failed to save debug screenshot '{}': {}", path.display(), error)
        })?;

    println!("Debug vision crop saved: {}", path.display());

    Ok(())
}
