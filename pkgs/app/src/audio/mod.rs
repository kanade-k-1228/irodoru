//! Media decoding, normalization and validation; playback is desktop-only.
mod import;
mod metadata;
mod wav;
pub use import::{AUDIO_EXTENSIONS, import_audio};
pub(crate) use metadata::{import_recording, read_voice, with_voice};
pub use wav::validate_wav;
#[cfg(feature = "desktop")]
mod playback;
#[cfg(feature = "desktop")]
pub(crate) use playback::Playback;
