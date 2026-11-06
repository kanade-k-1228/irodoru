//! Media decoding, normalization and validation; playback is desktop-only.
mod import;
mod wav;
pub use import::{AUDIO_EXTENSIONS, IMAGE_EXTENSIONS, import_audio, import_icon};
pub use wav::validate_wav;
#[cfg(feature = "desktop")]
mod playback;
#[cfg(feature = "desktop")]
pub(crate) use playback::Playback;
