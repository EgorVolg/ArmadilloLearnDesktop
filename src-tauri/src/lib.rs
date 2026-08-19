mod app;
mod app_core;

use app::runtime::AppRuntime;
use tauri::Manager;

pub fn run() {
    dotenv::dotenv().ok();

    tauri::Builder
        ::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let runtime = AppRuntime::new(app.handle().clone());

            app.manage(runtime);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
