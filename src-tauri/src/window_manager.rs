use mouse_position::mouse_position::Mouse;
use tauri::{AppHandle, Manager, Monitor, WebviewWindow, Wry};
use tauri::{PhysicalPosition, PhysicalSize, Position, Size};

pub struct MonitorInfo {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub fn get_mouse_position() -> Result<[i32; 2], String> {
    match Mouse::get_mouse_position() {
        Mouse::Position { x, y } => Ok([x, y]),
        Mouse::Error => Err("Failed to get mouse position".to_string()),
    }
}

pub fn find_monitor_by_position(
    app: &AppHandle<Wry>,
    mouse_x: i32,
    mouse_y: i32,
) -> Option<Monitor> {
    let monitors = app.available_monitors().unwrap_or_default();

    monitors.into_iter().find(|m| {
        let pos = m.position();
        let size = m.size();
        mouse_x >= pos.x
            && mouse_x <= pos.x + size.width as i32
            && mouse_y >= pos.y
            && mouse_y <= pos.y + size.height as i32
    })
}

pub fn calculate_window_geometry(
    mouse_x: i32,
    mouse_y: i32,
    monitor: &MonitorInfo,
) -> WindowGeometry {
    let calc_width = (monitor.width as f64 * 0.3125) as u32;

    let calc_height = (calc_width as f64 * (500.0 / 800.0)) as u32;

    let x = mouse_x
        .max(monitor.x)
        .min(monitor.x + monitor.width as i32 - calc_width as i32);

    let y = if mouse_y < monitor.y + (monitor.height / 2) as i32 {
        (mouse_y + 20)
            .max(monitor.y)
            .min(monitor.y + monitor.height as i32 - calc_height as i32)
    } else {
        (mouse_y - calc_height as i32 - 20)
            .max(monitor.y)
            .min(monitor.y + monitor.height as i32 - calc_height as i32)
    };

    WindowGeometry {
        x,
        y,
        width: calc_width,
        height: calc_height,
    }
}

pub fn get_monitor_info(app: &AppHandle<Wry>, mouse_x: i32, mouse_y: i32) -> MonitorInfo {
    if let Some(monitor) = find_monitor_by_position(app, mouse_x, mouse_y) {
        let pos = monitor.position();
        let size = monitor.size();
        MonitorInfo {
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
        }
    } else {
        let fallback = app.state::<MonitorInfo>();
        MonitorInfo {
            x: fallback.x,
            y: fallback.y,
            width: fallback.width,
            height: fallback.height,
        }
    }
}

pub fn reposition_and_show(app: &AppHandle<Wry>, window: &WebviewWindow) -> Result<(), String> {
    let mouse_pos = get_mouse_position().unwrap_or([0, 0]);
    let mouse_x = mouse_pos[0];
    let mouse_y = mouse_pos[1];

    let monitor = get_monitor_info(app, mouse_x, mouse_y);

    let geometry = calculate_window_geometry(mouse_x, mouse_y, &monitor);

    window
        .set_size(Size::Physical(PhysicalSize {
            width: geometry.width,
            height: geometry.height,
        }))
        .map_err(|e| e.to_string())?;

    window
        .set_position(Position::Physical(PhysicalPosition {
            x: geometry.x,
            y: geometry.y,
        }))
        .map_err(|e| e.to_string())?;

    window.show().map_err(|e| e.to_string())?;

    window.set_focus().map_err(|e| e.to_string())?;

    Ok(())
}