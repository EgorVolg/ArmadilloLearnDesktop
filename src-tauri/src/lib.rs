mod ai;
mod mouse_hook;
mod ocr;
mod window_manager;

use std::sync::mpsc;

use ai::translate;
use mouse_hook::MouseHook;

use ocr::{capture_area, get_word_at_position, ocr_from_png_with_words};

use tauri::Manager;

use window_manager::MonitorInfo;

use crate::ai::TranslationResult;

const OCR_WIDTH: u32 = 400;
const OCR_HEIGHT: u32 = 100;

const OCR_CENTER_X: f32 = OCR_WIDTH as f32 / 2.0;
const OCR_CENTER_Y: f32 = OCR_HEIGHT as f32 / 2.0;

#[tauri::command]
async fn translate_text(full_text: String, word: String) -> Result<TranslationResult, String> {
    ai::translate(&full_text, &word).await
}

pub fn run() {
    dotenv::dotenv().ok();

    tauri::Builder::default()
        .setup(|app| {
            //
            // Monitor info
            //

            if let Some(monitor) = app.primary_monitor()? {
                let size = monitor.size();
                let position = monitor.position();

                app.manage(MonitorInfo {
                    x: position.x,
                    y: position.y,
                    width: size.width,
                    height: size.height,
                });
            } else {
                app.manage(MonitorInfo {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                });
            }

            //
            // Mouse hook
            //

            let app_handle = app.handle().clone();

            let (tx, rx) = mpsc::channel::<(i32, i32)>();

            let mouse_hook = MouseHook::install(tx).expect("Failed to install mouse hook");

            //
            // Сохраняем hook в Tauri state.
            //
            // Иначе Drop сработает после setup
            // и hook будет снят.
            //

            app.manage(mouse_hook);

            //
            // Click processing thread
            //

            std::thread::spawn(move || {
                for (x, y) in rx {
                    println!("Middle click at ({},{})", x, y);

                    if let Some(png_bytes) = capture_area(x, y, OCR_WIDTH, OCR_HEIGHT) {
                        
                        let app_handle = app_handle.clone();

                        tauri::async_runtime::spawn(async move {
                            match ocr_from_png_with_words(png_bytes).await {
                                Ok(words) => {
                                    // Собираем полный текст из всех слов для отправки в AI
                                    let full_text: String = words
                                        .iter()
                                        .map(|w| w.text.as_str())
                                        .collect::<Vec<&str>>()
                                        .join(" ");

                                    println!("OCR: {}", full_text);

                                    if let Some(word) =
                                        get_word_at_position(&words, OCR_CENTER_X, OCR_CENTER_Y)
                                    {
                                        let full_text_clone = full_text.clone();

                                        let translation_result = match translate(&full_text_clone, &word).await {
                                            Ok(r) => {
                                                println!(
                                                    "Translation response: {{\"sentence\": \"{}\", \"word\": \"{}\", \"sentence_translation\": \"{}\", \"word_translation\": \"{}\", \"synonyms\": {:?}, \"part_of_speech\": \"{}\", \"topic\": \"{}\"}}",
                                                    full_text_clone,
                                                    word,
                                                    r.sentence_translation,
                                                    r.word_translation,
                                                    r.synonyms,
                                                    r.part_of_speech,
                                                    r.topic
                                                );
                                                r
                                            }
                                            Err(e) => {
                                                eprintln!("AI error: {}", e);
                                                ai::TranslationResult {
                                                    sentence_translation: "ошибка".into(),
                                                    word_translation: "ошибка".into(),
                                                    synonyms: vec![],
                                                    part_of_speech: "".into(),
                                                    topic: "".into(),
                                                }
                                            }
                                        };

                                        if let Some(window) = app_handle.get_webview_window("main")
                                        {
                                            let _ = window_manager::reposition_and_show(
                                                &app_handle,
                                                &window,
                                            );

                                            //
                                            // Отправляем полные данные перевода
                                            //

                                            let payload = serde_json::json!({
                                                "sentence": full_text_clone,
                                                "word": word,
                                                "sentence_translation": translation_result.sentence_translation,
                                                "word_translation": translation_result.word_translation,
                                                "synonyms": translation_result.synonyms,
                                                "part_of_speech": translation_result.part_of_speech,
                                                "topic": translation_result.topic,
                                            });
                                            let json_str = payload.to_string();

                                            let js = format!(
                                                "
                                                    window.__translationData = {};
                                                    window.dispatchEvent(
                                                        new Event(
                                                            'translationDataReady'
                                                        )
                                                    );
                                                    ",
                                                json_str
                                            );

                                            let _ = window.eval(&js);
                                        }
                                    } else {
                                        if let Some(window) = app_handle.get_webview_window("main")
                                        {
                                            let _ = window.hide();
                                        }
                                    }
                                }

                                Err(e) => {
                                    eprintln!("OCR error: {}", e);

                                    if let Some(window) = app_handle.get_webview_window("main") {
                                        let _ = window.hide();
                                    }
                                }
                            }
                        });
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}