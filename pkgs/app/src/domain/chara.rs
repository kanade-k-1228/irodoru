use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

pub const DEFAULT_CHARA_COLOR: &str = "#41a2ec";

#[derive(Clone, PartialEq, Deserialize, Serialize)]
pub struct Chara {
    pub id: String,
    pub name: String,
    pub description: String,
    pub voice_ids: Vec<String>,
    pub color: String,
    pub icon: Option<String>,
}

#[derive(Clone, PartialEq)]
pub struct CharaInput {
    pub name: String,
    pub description: String,
    pub voice_ids: Vec<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
}

impl CharaInput {
    pub(crate) fn normalize(self, id: String) -> Result<Chara> {
        let name = self.name.trim().to_owned();
        if name.is_empty() || name.chars().count() > 64 || self.description.chars().count() > 2000 {
            return Err(anyhow!(
                "name must be 1–64 characters; description at most 2000",
            ));
        }
        let color = self.color.unwrap_or_else(|| DEFAULT_CHARA_COLOR.into());
        if color.len() != 7
            || !color.starts_with('#')
            || !color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
        {
            return Err(anyhow!("color must be #RRGGBB"));
        }
        let mut seen = std::collections::HashSet::new();
        let voice_ids: Vec<_> = self
            .voice_ids
            .into_iter()
            .filter(|id| !id.is_empty() && seen.insert(id.clone()))
            .collect();
        if voice_ids.iter().any(|id| !super::valid_id(id)) {
            return Err(anyhow!("参照音声 ID が不正です"));
        }
        Ok(Chara {
            id,
            name,
            description: self.description.trim().into(),
            color: color.to_ascii_lowercase(),
            voice_ids,
            icon: self.icon.filter(|v| !v.is_empty()),
        })
    }
}

impl From<Chara> for CharaInput {
    fn from(chara: Chara) -> Self {
        Self {
            name: chara.name,
            description: chara.description,
            voice_ids: chara.voice_ids,
            color: Some(chara.color),
            icon: chara.icon,
        }
    }
}
