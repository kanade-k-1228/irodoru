//! Project lifetime, portable ZIP archives and single-writer autosave.
mod active;
mod archive;
mod autosave;
#[cfg(any(feature = "desktop", test))]
pub(crate) mod incoming;
mod ownership;
mod session;
pub use active::Project;
use autosave::Saver;
pub use session::{ProjectStatus, Session};
use session::{SharedProject, status};
#[cfg(test)]
mod tests;
