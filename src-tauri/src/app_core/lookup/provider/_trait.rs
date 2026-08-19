use crate::app_core::lookup::types::LookupResult;

pub trait AiProvider: Send + Sync {
    fn lookup(&self, image_png: &[u8], prompt: &str) -> Result<LookupResult, String>;
}
