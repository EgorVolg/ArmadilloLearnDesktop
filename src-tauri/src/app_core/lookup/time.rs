use std::time::{ SystemTime, UNIX_EPOCH };

// Текущее время в миллисекундах от Unix epoch (для логов и имён файлов).
pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}