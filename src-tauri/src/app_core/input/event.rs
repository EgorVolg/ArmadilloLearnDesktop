#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    Lookup { x: i32, y: i32 },
}
