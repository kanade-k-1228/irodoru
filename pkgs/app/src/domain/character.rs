use serde::{Deserialize, Serialize};

pub const DEFAULT_CHARACTER_COLOR: &str = "#41a2ec";
pub const DEFAULT_SAMPLE_TEXT: &str = "こんにちは。よろしくお願いします。";
fn default_character_color() -> String {
    DEFAULT_CHARACTER_COLOR.into()
}
fn default_sample_text() -> String {
    DEFAULT_SAMPLE_TEXT.into()
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Character {
    pub id: String,
    pub name: String,
    pub description: String,
    pub voice: Option<String>,
    #[serde(default = "default_character_color")]
    pub color: String,
    #[serde(default = "default_sample_text")]
    pub sample_text: String,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterInput {
    pub name: String,
    pub description: String,
    pub voice: Option<String>,
    pub color: Option<String>,
    pub sample_text: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}
