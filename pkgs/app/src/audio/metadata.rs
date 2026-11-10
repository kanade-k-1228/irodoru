//! App-owned metadata in standard WAV INFO/ICMT, shared by projects and exports.
use crate::domain::Voice;
use anyhow::Result;
use std::io::{Read, Seek};

pub fn with_voice(wav: &[u8], voice: &Voice) -> Result<Vec<u8>> {
    #[derive(serde::Serialize)]
    struct Metadata<'a> {
        irodoru_voice: &'a Voice,
    }
    irodoru_engine::wav_metadata::with_comment(
        wav,
        &serde_json::to_string(&Metadata {
            irodoru_voice: voice,
        })?,
    )
}

pub fn read_voice(reader: &mut (impl Read + Seek)) -> Result<Option<Voice>> {
    let Some(comment) = irodoru_engine::wav_metadata::read_comment(reader)? else {
        return Ok(None);
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&comment) else {
        return Ok(None);
    };
    if let Some(voice) = value.get("irodoru_voice") {
        return Ok(Some(serde_json::from_value(voice.clone())?));
    }
    Ok(None)
}

pub fn import_recording(path: &std::path::Path) -> Result<Vec<u8>> {
    // Preserve embedded generation metadata when reimporting an exported WAV.
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
    {
        let mut file = std::fs::File::open(path)?;
        if read_voice(&mut file)?.is_some() {
            let bytes = std::fs::read(path)?;
            super::validate_wav(&bytes)?;
            return Ok(bytes);
        }
    }
    super::import_audio(path)
}
