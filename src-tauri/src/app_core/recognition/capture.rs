use super::context::RecognitionContext;

pub struct CaptureStage;

impl CaptureStage {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&self, mut ctx: RecognitionContext) -> RecognitionContext {
        println!("CaptureStage ({}, {})", ctx.cursor_x, ctx.cursor_y);

        // TODO:
        // Сделать снимок экрана
        // и сохранить его в ctx.screenshot

        ctx
    }
}
