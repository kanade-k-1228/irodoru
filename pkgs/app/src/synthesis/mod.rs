//! Inference lifecycle, generation jobs and deployable model resources.
pub mod generation;
mod inference;
mod model_import;
#[cfg(any(feature = "desktop", test))]
mod model_resources;
#[cfg(any(feature = "desktop", test))]
pub(crate) mod resources;
pub use inference::Inference;
