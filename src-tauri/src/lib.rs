// Объявление подмодулей — каждый в отдельном файле (ocr.rs, window_manager.rs)
mod ocr;
mod window_manager;

// Импортируем функции из модуля ocr для захвата области экрана, OCR и поиска слова
use ocr::{capture_area, get_word_at_position, ocr_from_png};
// Импорт трейта Manager из Tauri — даёт методы для управления окнами
use tauri::Manager;
// Импорт структуры MonitorInfo из модуля window_manager
use window_manager::MonitorInfo;

// Импорты для глобального хука мыши (низкоуровневый WinAPI hook)
use std::sync::{mpsc, Mutex};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, MSLLHOOKSTRUCT, WH_MOUSE_LL,
    WM_MBUTTONDOWN,
};

// Глобальная переменная для передачи координат из хука в основной поток
// Используется Mutex для безопасного доступа из нескольких потоков
static GLOBAL_TX: Mutex<Option<mpsc::Sender<(i32, i32)>>> = Mutex::new(None);

/// Низкоуровневый хук для мыши (WH_MOUSE_LL)
/// Вызывается системой при каждом событии мыши
unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // code >= 0 означает, что хук должен обработать сообщение
    if code >= 0 && wparam.0 == WM_MBUTTONDOWN as usize {
        // lparam указывает на структуру MSLLHOOKSTRUCT, содержащую детали события
        let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let x = info.pt.x;
        let y = info.pt.y;

        // Отправляем координаты через канал, если он существует
        if let Some(tx) = GLOBAL_TX.lock().unwrap().as_ref() {
            let _ = tx.send((x, y));
        }
    }
    // Передаём управление следующему хуку в цепочке
    CallNextHookEx(None, code, wparam, lparam)
}

// Главная функция запуска приложения
pub fn run() {
    tauri::Builder::default()
        // Настройка приложения при запуске
        .setup(|app| {
            // Информация о первичном мониторе
            if let Some(monitor) = app.primary_monitor()? {
                let size = monitor.size();
                let position = monitor.position();
                // Сохраняем информацию о мониторе в managed state
                app.manage(MonitorInfo {
                    x: position.x,
                    y: position.y,
                    width: size.width,
                    height: size.height,
                });
            } else {
                // Если монитор не найден, используем дефолтные значения 1920x1080
                app.manage(MonitorInfo {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                });
            }

            // --- Глобальный хук средней кнопки мыши через WinAPI ---
            let app_handle = app.handle().clone();

            // Канал для получения координат из хука
            let (tx, rx) = mpsc::channel::<(i32, i32)>();
            *GLOBAL_TX.lock().unwrap() = Some(tx);

            // Устанавливаем хук (WH_MOUSE_LL позволяет работать без инжекта DLL)
            unsafe {
                let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), Some(HINSTANCE::default()), 0)
                    .expect("Failed to set mouse hook");
                // Приводим HHOOK к isize, чтобы обойти ограничение Send (HHOOK = *mut c_void не Send)
                let hook_raw = hook.0 as isize;
                // Запускаем цикл обработки сообщений в отдельном потоке
                std::thread::spawn(move || {
                    let mut msg = std::mem::zeroed();
                    // Стандартный message loop (нужен для работы хука)
                    while windows::Win32::UI::WindowsAndMessaging::GetMessageW(&mut msg, None, 0, 0).into() {
                        let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                        windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
                    }
                    // При выходе из потока отключаем хук (восстанавливаем HHOOK из isize)
                    let hook = windows::Win32::UI::WindowsAndMessaging::HHOOK(hook_raw as *mut std::ffi::c_void);
                    let _ = UnhookWindowsHookEx(hook);
                });
            }

            // Поток для обработки кликов, полученных из канала
            std::thread::spawn(move || {
                for (x, y) in rx {
                    println!("Middle click at ({},{})", x, y);

                    // Захватываем область 250x50 вокруг точки клика
                    if let Some(png_bytes) = capture_area(x, y, 250, 50) {
                        let app_handle = app_handle.clone();
                        // Переключаемся в асинхронный контекст Tauri для вызова OCR
                        tauri::async_runtime::spawn(async move {
                            match ocr_from_png(png_bytes).await {
                                Ok(text) => {
                                    println!("OCR: {}", text);
                                    // Ищем первое слово в распознанном тексте
                                    if let Some(word) = get_word_at_position(&text, 125.0, 25.0) {
                                        // Слово найдено — показываем/обновляем окно
                                        if let Some(window) = app_handle.get_webview_window("main") {
                                            let _ = window_manager::reposition_and_show(&app_handle, &window);
                                            // Передаём данные OCR в окно через JS eval
                                            let _ = window.eval(&format!(
                                                "window.__translationData = {{ word: '{}' }}; window.dispatchEvent(new Event('translationDataReady'));",
                                                word
                                            ));
                                        }
                                    } else if let Some(window) = app_handle.get_webview_window("main") {
                                        // Пустое пространство — закрываем окно
                                        let _ = window.hide();
                                    }
                                }
                                Err(e) => {
                                    eprintln!("OCR error: {}", e);
                                    // При ошибке OCR тоже закрываем окно
                                    if let Some(window) = app_handle.get_webview_window("main") {
                                        let _ = window.hide();
                                    }
                                },
                            }
                        });
                    }
                }
            });

            Ok(())
        })
        // Запускаем приложение, передавая контекст, сгенерированный tauri-build
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
