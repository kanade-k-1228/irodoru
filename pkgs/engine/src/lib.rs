mod backend;
mod cache;
mod config;
mod engine;
mod error;
mod model;
mod output;
mod request;
pub mod wav_metadata;
#[cfg(target_os = "windows")]
pub mod windows_runtime;
pub use backend::{
    ProviderStatus, ProviderTracker, Runtime,
    device::{DeviceInfo, DevicePreference},
};
pub use config::EngineConfig;
pub use engine::Engine;
pub use error::AppError;
pub use model::{Audio, SynthOutput, SynthParams};
pub use output::GenerationConditions;
pub use request::{SynthesisRequest, TTSJob};
