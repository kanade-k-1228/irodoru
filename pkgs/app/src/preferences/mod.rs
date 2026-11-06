//! OS-local preferences and their ordered persistence worker.
mod store;
pub use store::{Settings, SettingsWriter, lock_instance};
#[cfg(feature = "desktop")]
pub(crate) mod theme;
