use crate::app_core::input::event::InputEvent;
use crate::app_core::overlay::manager::OverlayManager;

pub struct ClickPipeline {
    overlay: OverlayManager,
}

impl ClickPipeline {
    pub fn new(overlay: OverlayManager) -> Self {
        Self { overlay }
    }

    pub fn process(&self, event: InputEvent) {
        match event {
            InputEvent::Lookup { x, y } => {
                self.overlay.show(x, y);
            }
        }
    }
}
