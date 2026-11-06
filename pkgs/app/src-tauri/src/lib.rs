#[cfg(feature = "desktop")]
mod commands;
pub mod error;
pub mod generation;
pub mod library;
pub mod types;

pub mod inference;
#[cfg(feature = "desktop")]
mod resources;
#[cfg(feature = "desktop")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    commands::run();
}

pub mod model_resources;
