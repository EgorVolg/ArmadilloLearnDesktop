use tauri::AppHandle;

use crate::app_core::overlay::window::OverlayWindow;

pub struct OverlayManager {
    window: OverlayWindow,
}

impl OverlayManager {
    pub fn new(app: AppHandle) -> Self {
        Self {
            window: OverlayWindow::new(app),
        }
    }

    pub fn show(&self, x: i32, y: i32, highlight: Option<(f32, f32, f32, f32)>) {
        self.window.show(x, y, highlight);
    }

    pub fn hide(&self) {
        self.window.hide();
    }
}
