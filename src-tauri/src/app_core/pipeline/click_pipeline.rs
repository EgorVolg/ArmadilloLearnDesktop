use std::sync::Arc;

use crate::app_core::input::event::InputEvent;
use crate::app_core::overlay::manager::OverlayManager;
use crate::app_core::recognition::context::RecognitionContext;

pub struct ClickPipeline {
    overlay: Arc<OverlayManager>,
}

impl ClickPipeline {
    pub fn new(overlay: Arc<OverlayManager>) -> Self {
        Self { overlay }
    }

    pub fn process(&self, event: InputEvent) {
        match event {
            InputEvent::Lookup { x, y } => {
                let ctx = RecognitionContext::new(x, y);

                // let ctx = self.capture.run(ctx);

                // let ctx = self.ocr.run(ctx);

                // let ctx = self.translate.run(ctx);

                self.overlay.show(ctx.cursor_x, ctx.cursor_y);
            }
        }
    }
}
