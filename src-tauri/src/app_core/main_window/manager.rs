use tauri::{AppHandle, Manager};

pub struct OverlayWindow {
    app: AppHandle,
}

impl OverlayWindow {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    pub fn show(&self) {
        if let Some(window) = self.app.get_webview_window("overlay") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }

    pub fn hide(&self) {
        if let Some(window) = self.app.get_webview_window("overlay") {
            let _ = window.hide();
        }
    }
}
