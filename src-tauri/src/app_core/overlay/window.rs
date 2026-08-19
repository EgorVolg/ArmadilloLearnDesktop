use tauri::{AppHandle, Manager, PhysicalPosition, WebviewWindow};

#[derive(Clone)]
pub struct OverlayWindow {
    app: AppHandle,
}

impl OverlayWindow {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    fn window(&self) -> WebviewWindow {
        self.app
            .get_webview_window("main")
            .expect("Main window not found")
    }

    pub fn show(&self, x: i32, y: i32) {
        let window = self.window();

        let _ = window.set_position(PhysicalPosition::new(x + 20, y + 20));
        let _ = window.show();
        let _ = window.set_focus();
    }

    pub fn hide(&self) {
        let window = self.window();

        let _ = window.hide();
    }
}
