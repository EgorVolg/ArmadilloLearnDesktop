mod app;
mod app_core;

use app::runtime::AppRuntime;
use tauri::{Manager, WindowEvent};

#[tauri::command]
fn open_main_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .show()
            .map_err(|e| format!("Failed to show main window: {e}"))?;

        window
            .set_focus()
            .map_err(|e| format!("Failed to focus main window: {e}"))?;

        return Ok(());
    }

    tauri::WebviewWindowBuilder::new(
        &app,
        "main",
        tauri::WebviewUrl::App("index.html#/main".into()),
    )
    .title("Armadillo Learn")
    .inner_size(1000.0, 700.0)
    .resizable(true)
    .build()
    .map_err(|e| format!("Failed to create main window: {e}"))?;

    Ok(())
}

pub fn run() {
    dotenv::dotenv().ok();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = open_main_window(app.clone());
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let runtime = AppRuntime::new(app.handle().clone());
            app.manage(runtime);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![open_main_window])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            // Закрытие main через стандартный крестик.
            if let tauri::RunEvent::WindowEvent {
                event: WindowEvent::CloseRequested { .. },
                ..
            } = &event
            {}
        });
}
