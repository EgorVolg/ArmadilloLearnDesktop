use screenshots::Screen;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct MonitorInfo {
    pub index: usize,

    pub x: i32,
    pub y: i32,

    pub width: u32,
    pub height: u32,

    pub scale_factor: f32,
}

impl MonitorInfo {
    pub fn contains_global_point(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && y >= self.y
            && x < self.x + self.width as i32
            && y < self.y + self.height as i32
    }

    pub fn global_to_local(&self, x: i32, y: i32) -> (i32, i32) {
        (x - self.x, y - self.y)
    }
}

pub struct MonitorManager;

impl MonitorManager {
    pub fn all() -> Result<Vec<MonitorInfo>, String> {
        let screens = Screen::all()
            .map_err(|e| format!("failed to enumerate screens: {e}"))?;

        screens
            .iter()
            .enumerate()
            .map(|(index, screen)| {
                let display_info = screen.display_info;

                Ok(MonitorInfo {
                    index,
                    x: display_info.x,
                    y: display_info.y,
                    width: display_info.width,
                    height: display_info.height,
                    scale_factor: display_info.scale_factor as f32,
                })
            })
            .collect()
    }

    pub fn find_at(x: i32, y: i32) -> Result<MonitorInfo, String> {
        Self::all()?
            .into_iter()
            .find(|monitor| monitor.contains_global_point(x, y))
            .ok_or_else(|| {
                format!(
                    "no monitor contains global point ({x}, {y})"
                )
            })
    }
}