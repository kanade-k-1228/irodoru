//! Shared value contracts. No GUI, filesystem or service dependencies.
mod character;
mod error;
mod id;
mod script;
mod speech;
mod take;
mod voice;
pub use character::{Character, CharacterInput, DEFAULT_CHARACTER_COLOR, DEFAULT_SAMPLE_TEXT};
pub use error::AppError;
pub use id::new_id;
pub(crate) use id::valid_id;
pub use script::{EditorState, GenerationSettings, ScriptRow};
pub use speech::{InferenceStatus, SynthesisInput, TakeRequest};
pub use take::Take;
pub use voice::Voice;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioSource {
    Take(String),
    Preview(String),
    CharacterPreview(String),
}
