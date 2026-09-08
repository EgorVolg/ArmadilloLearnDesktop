#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    /// Показать оверлей по координатам курсора (Ctrl+P или средняя кнопка мыши).
    Lookup { x: i32, y: i32 },

    /// Любое другое нажатие клавиши/кнопки мыши — скрыть оверлей.
    Dismiss,
}
