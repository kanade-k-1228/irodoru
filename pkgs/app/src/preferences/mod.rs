//! OS-local preferences and their ordered persistence worker.
mod models;
mod store;
pub use models::ModelSettings;
pub use store::{Settings, SettingsWriter, Theme, lock_instance};
pub mod theme;
