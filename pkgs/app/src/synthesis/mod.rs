//! Inference lifecycle, generation jobs and deployable model resources.
pub mod generation;
mod inference;
mod model_import;
mod model_resources;
pub(crate) mod resources;
pub use inference::Inference;
