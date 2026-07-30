use super::{context::RecognitionContext, region::TextRegion};

pub struct DetectionStage;

impl DetectionStage {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&self, mut ctx: RecognitionContext) -> RecognitionContext {
        println!("DetectionStage");

        //
        // TODO
        //
        // Найти все области текста
        //

        ctx.regions = Vec::<TextRegion>::new();

        ctx
    }
}
