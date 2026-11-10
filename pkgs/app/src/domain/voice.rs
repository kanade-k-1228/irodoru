use serde::{Deserialize, Serialize};

use super::Take;

/// Imported recordings and generated takes share one audio library and ID space.
#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Voice {
    Imported { id: String, name: String },
    Generated(Box<Take>),
}

impl Voice {
    pub fn id(&self) -> &str {
        match self {
            Self::Imported { id, .. } => id,
            Self::Generated(take) => &take.id,
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Imported { name, .. } => name.clone(),
            Self::Generated(take) => format!("{}: {}", take.chara_name, take.conditions.text),
        }
    }
}
