mod mouse_hook;
mod ocr;
mod window_manager;

use std::sync::mpsc;

use mouse_hook::MouseHook;

use ocr::{capture_area, get_word_at_position, ocr_from_png};

use tauri::Manager;

use window_manager::MonitorInfo;

const OCR_WIDTH: u32 = 250;
const OCR_HEIGHT: u32 = 50;

const OCR_CENTER_X: f32 = OCR_WIDTH as f32 / 2.0;
const OCR_CENTER_Y: f32 = OCR_HEIGHT as f32 / 2.0;

pub fn run() {
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
                            match ocr_from_png(png_bytes).await {
                                Ok(text) => {
                                    println!("OCR: {}", text);

                                    if let Some(word) =
                                        get_word_at_position(&text, OCR_CENTER_X, OCR_CENTER_Y)
                                    {
                                        if let Some(window) = app_handle.get_webview_window("main")
                                        {
                                            let _ = window_manager::reposition_and_show(
                                                &app_handle,
                                                &window,
                                            );

                                            //
                                            // Безопасный JSON вместо
                                            // ручной строки JS
                                            //

                                            let word_json = serde_json::to_string(&word).unwrap();

                                            let js = format!(
                                                "
                                                    window.__translationData = {{
                                                        word: {}
                                                    }};

                                                    window.dispatchEvent(
                                                        new Event(
                                                            'translationDataReady'
                                                        )
                                                    );
                                                    ",
                                                word_json
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
