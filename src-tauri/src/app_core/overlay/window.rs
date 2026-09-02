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

    pub fn show(&self, x: i32, y: i32) {
        let window = self.window();

        let _ = window.set_position(PhysicalPosition::new(x + 20, y + 20));

        // Никакого set_focus(): оверлей не должен забирать фокус у активного
        // приложения — fullscreen-плеер при потере фокуса сворачивается.
        //
        // window.show() в Tauri использует SW_SHOW и АКТИВИРУЕТ окно, поэтому
        // на Windows показываем напрямую: ShowWindow(SW_SHOWNA) — показать
        // без активации, плюс SetWindowPos(HWND_TOPMOST) как страховка z-order
        // поверх всех окон (тоже без активации).
        #[cfg(target_os = "windows")]
        raise_without_activation(&window);

        #[cfg(not(target_os = "windows"))]
        let _ = window.show();
    }

    /// Находится ли точка (физические координаты экрана) внутри окна оверлея.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        let window = self.window();

        let Ok(position) = window.outer_position() else {
            return false;
        };

        let Ok(size) = window.outer_size() else {
            return false;
        };

        point_in_rect(
            x,
            y,
            position.x,
            position.y,
            size.width as i32,
            size.height as i32,
        )
    }

    pub fn hide(&self) {
        let window = self.window();
        let _ = window.hide();
    }
}

/// Показывает окно и поднимает поверх всех окон, НЕ активируя его.
///
/// Обычный `show()` (SW_SHOW) делает окно активным: активное приложение
/// теряет фокус, и Windows сворачивает fullscreen-плеер. SW_SHOWNA +
/// SWP_NOACTIVATE показывают окно «пассивно» — фокус остаётся у плеера.
#[cfg(target_os = "windows")]
fn raise_without_activation(window: &WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, ShowWindow, HWND_TOPMOST, SW_SHOWNA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SWP_SHOWWINDOW,
    };

    let Ok(raw) = window.hwnd() else {
        return;
    };

    // HWND из версии windows-крейта, которую использует Tauri, приводим
    // к нашему типу; cast работает и для isize-, и для pointer-repr.
    let hwnd = HWND(raw.0 as *mut core::ffi::c_void);

    unsafe {
        // SW_SHOWNA: показать в текущих размере/позиции без активации.
        let _ = ShowWindow(hwnd, SW_SHOWNA);

        // Страховка z-order поверх всех окон (alwaysOnTop из конфига уже
        // даёт TOPMOST, это повторное подтверждение при каждом показе);
        // NOACTIVATE — не забирать фокус, NOMOVE/NOSIZE — не двигать.
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

/// Попадание точки (физические координаты экрана) в прямоугольник.
/// Правый/нижний край не считаются частью окна — клик ровно у границы
/// не должен цеплять оверлей.
fn point_in_rect(x: i32, y: i32, rect_x: i32, rect_y: i32, width: i32, height: i32) -> bool {
    x >= rect_x
        && x < rect_x.saturating_add(width)
        && y >= rect_y
        && y < rect_y.saturating_add(height)
}

#[cfg(test)]
mod tests {
    use super::point_in_rect;

    #[test]
    fn point_inside_rect() {
        assert!(point_in_rect(150, 200, 100, 100, 300, 200));
    }

    #[test]
    fn point_outside_rect() {
        assert!(!point_in_rect(99, 200, 100, 100, 300, 200));
        assert!(!point_in_rect(400, 200, 100, 100, 300, 200));
        assert!(!point_in_rect(150, 99, 100, 100, 300, 200));
        assert!(!point_in_rect(150, 300, 100, 100, 300, 200));
    }

    #[test]
    fn left_top_edge_is_inside_right_bottom_is_not() {
        assert!(point_in_rect(100, 100, 100, 100, 300, 200));
        assert!(!point_in_rect(400, 150, 100, 100, 300, 200));
        assert!(!point_in_rect(150, 300, 100, 100, 300, 200));
    }

    #[test]
    fn zero_size_rect_never_contains() {
        assert!(!point_in_rect(100, 100, 100, 100, 0, 0));
    }
}
