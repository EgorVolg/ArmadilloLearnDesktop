use mouse_position::mouse_position::Mouse;
use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

// Структура для хранения информации о мониторе
struct MonitorInfo {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[tauri::command]
fn show_translation() {
    println!("show_translation");
}

#[tauri::command]
fn get_mouse_position() -> Result<[i32; 2], String> {
    match Mouse::get_mouse_position() {
        Mouse::Position { x, y } => Ok([x, y]),
        Mouse::Error => Err("Failed to get mouse position".to_string()),
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        if let Some(window) = app.get_webview_window("main") {
                            // Если окно видимо — прячем его
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let mouse_pos = get_mouse_position().unwrap_or([0, 0]);
                                let mouse_x = mouse_pos[0];
                                let mouse_y = mouse_pos[1];

                                // Определяем монитор, на котором находится мышь
                                let monitors = app.available_monitors().unwrap_or_default();
                                let target_monitor = monitors.iter().find(|m| {
                                    let pos = m.position();
                                    let size = m.size();
                                    mouse_x >= pos.x
                                        && mouse_x <= pos.x + size.width as i32
                                        && mouse_y >= pos.y
                                        && mouse_y <= pos.y + size.height as i32
                                });

                                // Используем найденный монитор или fallback на primary
                                let (mon_x, mon_y, mon_width, mon_height) =
                                    if let Some(monitor) = target_monitor {
                                        let pos = monitor.position();
                                        let size = monitor.size();
                                        (pos.x, pos.y, size.width, size.height)
                                    } else {
                                        let fallback = app.state::<MonitorInfo>();
                                        (fallback.x, fallback.y, fallback.width, fallback.height)
                                    };

                                let calc_width = (mon_width as f64 * 0.3125) as u32;
                                let calc_height = (calc_width as f64 * (500.0 / 800.0)) as u32;

                                // Корректируем позицию, чтобы окно не выходило за границы монитора
                                let x = mouse_x
                                    .max(mon_x)
                                    .min(mon_x + mon_width as i32 - calc_width as i32);

                                // Если курсор в верхней половине — окно под курсором, иначе — над курсором
                                let y = if mouse_y < mon_y + (mon_height / 2) as i32 {
                                    // Верхняя половина: окно под курсором + отступ 20px
                                    (mouse_y + 20)
                                        .max(mon_y)
                                        .min(mon_y + mon_height as i32 - calc_height as i32)
                                } else {
                                    // Нижняя половина: окно над курсором - отступ 20px
                                    (mouse_y - calc_height as i32 - 20)
                                        .max(mon_y)
                                        .min(mon_y + mon_height as i32 - calc_height as i32)
                                };

                                let _ =
                                    window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
                                        width: calc_width,
                                        height: calc_height,
                                    }));

                                let _ = window.set_position(tauri::Position::Physical(
                                    tauri::PhysicalPosition { x, y },
                                ));
                                let _ = window.show();
                                let _ = window.set_focus();
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
