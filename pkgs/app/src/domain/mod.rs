//! Shared value contracts. No GUI, filesystem or service dependencies.
mod chara;
mod id;
mod line;
mod speech;
mod take;
mod voice;
pub use chara::{Chara, CharaInput, DEFAULT_CHARA_COLOR, DEFAULT_SAMPLE_TEXT};
pub use id::new_id;
pub(crate) use id::valid_id;
pub use line::{EditorState, GenerationSettings, Line};
pub use speech::{InferenceStatus, SynthesisInput, TakeRequest};
pub use take::Take;
pub use voice::Voice;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioSource {
    Take(String),
    CharaPreview(String),
}
