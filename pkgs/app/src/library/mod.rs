//! Filesystem repository for characters, references, takes and the editable document.
mod repository;
pub use repository::Library;

pub(crate) use repository::valid_script_id;
