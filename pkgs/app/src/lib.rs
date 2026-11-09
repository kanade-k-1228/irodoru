//! Desktop application capabilities; core services also compile without a GUI.
pub mod audio;
pub mod domain;
mod files;
mod icons;
pub mod library;
pub mod preferences;
pub mod project;
pub mod synthesis;
pub mod workspace;

#[cfg(feature = "desktop")]
extern crate gpui_kit as gpui;
#[cfg(feature = "desktop")]
pub mod desktop;
