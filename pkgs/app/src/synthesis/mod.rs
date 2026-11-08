//! Inference lifecycle, generation jobs and deployable model resources.
pub mod generation;
mod inference;
mod model_resources;
pub mod resources;
pub use inference::Inference;
