//! Inference lifecycle, generation jobs and deployable model resources.
pub mod bundle;
pub mod generation;
mod inference;
pub mod resources;
pub use inference::Inference;
