use anyhow::Result;
use std::{path::PathBuf, time::Instant};

pub(super) struct Stopwatch {
    mark: Instant,
    pub(super) laps: Vec<(&'static str, f64)>,
}

impl Stopwatch {
    pub(super) fn start() -> Self {
        Self {
            mark: Instant::now(),
            laps: Vec::new(),
        }
    }

    pub(super) fn lap(&mut self, name: &'static str) {
        self.laps
            .push((name, self.mark.elapsed().as_secs_f64() * 1000.0));
        self.mark = Instant::now();
    }
}

pub(super) struct DebugDump(PathBuf);

impl DebugDump {
    pub(super) fn from_env() -> Option<Self> {
        std::env::var_os("IRODORI_DEBUG_DUMP").map(|dir| Self(PathBuf::from(dir)))
    }

    pub(super) fn f32<'a>(
        &self,
        name: &str,
        data: impl IntoIterator<Item = &'a f32>,
    ) -> Result<()> {
        std::fs::create_dir_all(&self.0)?;
        let bytes: Vec<u8> = data.into_iter().flat_map(|v| v.to_le_bytes()).collect();
        Ok(std::fs::write(self.0.join(name), bytes)?)
    }

    pub(super) fn json(&self, name: &str, value: &serde_json::Value) -> Result<()> {
        std::fs::create_dir_all(&self.0)?;
        Ok(std::fs::write(self.0.join(name), value.to_string())?)
    }
}
