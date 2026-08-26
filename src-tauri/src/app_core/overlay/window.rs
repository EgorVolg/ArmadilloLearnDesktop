use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow};

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
            .get_webview_window("overlay")
            .expect("Overlay window not found")
    }

    pub fn show(&self, x: i32, y: i32, highlight: Option<(f32, f32, f32, f32)>) {
        let window = self.window();

        let _ = window.set_position(PhysicalPosition::new(x + 20, y + 20));

        if let Some(bbox) = highlight {
            let _ = window.emit("ocr-highlight", bbox);
        }

        let _ = window.show();
        let _ = window.set_focus();
    }

    pub fn hide(&self) {
        let window = self.window();
        let _ = window.hide();
    }
}
