//! Project lifetime, portable ZIP archives and single-writer autosave.
mod archive;
mod autosave;
mod ownership;
pub(crate) use ownership::WindowProject;
mod session;
pub use autosave::Saver;
pub use session::{ProjectStatus, Session, SharedProject, status};
#[cfg(test)]
mod tests;
