use tauri::AppHandle;

use super::window::MainWindow;

pub struct MainWindowManager {
    window: MainWindow,
}

impl MainWindowManager {
    pub fn new(app: AppHandle) -> Self {
        Self {
            window: MainWindow::new(app),
        }
    }

    pub fn show(&self) {
        self.window.show();
    }

    pub fn hide(&self) {
        self.window.hide();
    }
}
