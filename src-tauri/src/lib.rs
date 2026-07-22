mod window_manager;

use window_manager::MonitorInfo;
use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[tauri::command]
fn show_translation() {
    println!("show_translation");
}

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window_manager::reposition_and_show(app, &window);
                            }
                        }
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![show_translation])
        .setup(|app| {
            // Получаем первичный монитор и его размер
            if let Some(monitor) = app.primary_monitor()? {
                let size = monitor.size();
                let position = monitor.position();
                // Сохраняем информацию о мониторе в состоянии Tauri
                app.manage(MonitorInfo {
                    x: position.x,
                    y: position.y,
                    width: size.width,
                    height: size.height,
                });
            } else {
                // Если монитор не найден — используем значения по умолчанию
                app.manage(MonitorInfo {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                });
            }

            let shortcut = Shortcut::new(Some(Modifiers::CONTROL), Code::KeyP);
            app.global_shortcut().register(shortcut)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
