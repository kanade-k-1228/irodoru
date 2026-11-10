use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GenerationSettings {
    pub seed: String,
    pub num_steps: String,
    pub cfg_text: f32,
    pub cfg_speaker: f32,
    pub cfg_caption: f32,
    pub duration_scale: f64,
}
impl GenerationSettings {
    /// The UI shows speaking speed; projects and inference store duration.
    pub fn speed(&self) -> f64 {
        self.duration_scale.recip()
    }

    pub fn set_speed(&mut self, speed: f64) {
        self.duration_scale = speed.recip();
    }
}
impl Default for GenerationSettings {
    fn default() -> Self {
        Self {
            seed: String::new(),
            num_steps: String::new(),
            cfg_text: 3.0,
            cfg_speaker: 5.0,
            cfg_caption: 3.0,
            duration_scale: 1.0,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Line {
    pub takes: Vec<String>,
    pub take_id: Option<String>,
    pub id: String,
    pub text: String,
    pub chara_id: String,
    pub prompt: Option<String>,
    pub settings: Option<GenerationSettings>,
}
#[derive(Clone, Debug, Default)]
pub struct EditorState {
    pub lines: Vec<Line>,
    pub active_chara: String,
}
