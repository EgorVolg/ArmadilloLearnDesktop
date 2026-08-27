use crate::app_core::lookup::types::LookupResult;

pub trait AiProvider: Send + Sync {
    fn lookup(&self, sentence: &str, word: &str) -> Result<LookupResult, String>;
}
