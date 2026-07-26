mod api;
mod mouse_hook;
mod screenshot;
mod window_manager;

use mouse_hook::MouseHook;

use tauri::Manager;

use window_manager::{reposition_and_show, MonitorInfo};

use screenshot::{
    capture_area, capture_area_bytes, capture_full_screen, capture_full_screen_bytes,
};

use api::fetch_translation_from_api;
use serde_json;
use tauri::Emitter;

pub fn run() {
    dotenv::dotenv().ok();

    tauri::Builder::default()
        .setup(|app| {
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

            let (tx, rx) = std::sync::mpsc::channel::<(i32, i32)>();

            let mouse_hook = MouseHook::install(tx).expect("Failed to install mouse hook");

            app.manage(mouse_hook);

            let app_handle = app.handle().clone();

            std::thread::spawn(move || {
                while let Ok((x, y)) = rx.recv() {
                    // Показываем окно при клике
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = reposition_and_show(&app_handle, &window);
                    }

                    // Сохраняем скриншоты на диск для отладки
                    if let Err(e) = capture_full_screen(None) {
                        eprintln!("Full screen save error: {}", e);
                    }

                    let area_x = x - 100;
                    let area_y = y - 100;
                    if let Err(e) = capture_area(area_x, area_y, 200, 200, None) {
                        eprintln!("Area save error: {}", e);
                    }

                    // Делаем скриншот всего экрана (в память для API)
                    let full_bytes = match capture_full_screen_bytes() {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            eprintln!("Full screen capture error: {}", e);
                            continue;
                        }
                    };

                    // Делаем скриншот области 200x200 вокруг курсора (в память для API)
                    let area_bytes = match capture_area_bytes(area_x, area_y, 200, 200) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            eprintln!("Area capture error: {}", e);
                            continue;
                        }
                    };

                    // Кодируем в base64
                    let full_b64 = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        &full_bytes,
                    );
                    let area_b64 = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        &area_bytes,
                    );

                    // Отправляем в API
                    match fetch_translation_from_api(&full_b64, "image/png", &area_b64, "image/png")
                    {
                        Ok(result) => {
                            println!("=== Translation Result ===");
                            println!("Sentence: {}", result.sentence_translation);
                            println!("Word: {}", result.word_translation);
                            println!("Synonyms: {:?}", result.synonyms);
                            println!("POS: {}", result.part_of_speech);
                            println!("Topic: {}", result.topic);
                            println!("==========================");

                            // Отправляем данные на фронтенд
                            let payload = serde_json::to_value(&result).unwrap_or_default();
                            let _ = app_handle.emit("translationDataReady", payload);
                        }
                        Err(e) => {
                            eprintln!("API error: {}", e);
                        }
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
