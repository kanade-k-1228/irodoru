//! Project workspace: editing rules, state, ordered commands and presentation.
pub mod editor;
pub mod protocol;
pub mod service;
pub mod state;
#[cfg(feature = "desktop")]
pub(crate) mod view;
mod worker;
