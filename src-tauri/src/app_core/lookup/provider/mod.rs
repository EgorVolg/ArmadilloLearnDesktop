// pub mod gemini;
pub mod groq;
pub mod local;

pub mod _trait;

// pub use gemini::GeminiProvider;
pub use groq::build_provider;
pub use local::LocalProvider;
