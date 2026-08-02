use tauri::{AppHandle, Manager};

pub struct MainWindow {
    app: AppHandle,
}

impl MainWindow {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    pub fn show(&self) {
        if let Some(window) = self.app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }

    pub fn hide(&self) {
        if let Some(window) = self.app.get_webview_window("main") {
            let _ = window.hide();
        }
    }
}
