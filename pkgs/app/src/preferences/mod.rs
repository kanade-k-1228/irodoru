//! OS-local preferences.
mod models;
mod store;
pub use models::ModelSettings;
pub use store::{Settings, Theme, lock_instance};
pub mod theme;
