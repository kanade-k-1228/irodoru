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

#[cfg(test)]
mod tests {
    use super::*;
    use irodoru_engine::{Audio, GenerationConditions, SynthesisRequest, TTSJob};

    #[test]
    fn standalone_export_keeps_conditions_when_imported_without_a_sidecar() {
        let job: TTSJob = SynthesisRequest {
            text: "日本語のセリフ".into(),
            seed: Some(42),
            num_steps: Some(8),
            ..Default::default()
        }
        .try_into()
        .unwrap();
        let conditions =
            GenerationConditions::from_params("anime".into(), &job.params, 42, 8, 48000);
        let voice = Voice::Generated(Box::new(crate::domain::Take {
            id: crate::domain::new_id(),
            row_id: crate::domain::new_id(),
            character_id: None,
            character_name: "生成音声".into(),
            additional_prompt: String::new(),
            voice_ids: Vec::new(),
            conditions: conditions.clone(),
            elapsed_ms: 100,
            audio_seconds: 1.0,
            created_at: 0,
        }));
        let wav = with_voice(
            &Audio::new(vec![0.1; 48000], 48000).to_wav().unwrap(),
            &voice,
        )
        .unwrap();
        let mut incomplete = serde_json::json!({"irodoru_voice": &voice});
        incomplete["irodoru_voice"]
            .as_object_mut()
            .unwrap()
            .remove("conditions");
        let incomplete_wav =
            irodoru_engine::wav_metadata::with_comment(&wav, &incomplete.to_string()).unwrap();
        assert!(read_voice(&mut std::io::Cursor::new(incomplete_wav)).is_err());
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("export.wav");
        std::fs::write(&file, &wav).unwrap();
        assert_eq!(import_recording(&file).unwrap(), wav);
        let library = crate::library::Library::open(&dir.path().join("project")).unwrap();
        let id = library.save_voice("export", &wav).unwrap();
        let Voice::Generated(take) = library.voice_record(&id).unwrap() else {
            panic!("generation metadata lost");
        };
        assert_eq!(take.id, id);
        assert_eq!(take.conditions.text, "日本語のセリフ");
        assert_eq!(take.conditions.seed, 42);
        assert_eq!(take.audio_seconds, 1.0);
        assert_eq!(
            serde_json::to_value(&take.conditions).unwrap(),
            serde_json::to_value(&conditions).unwrap()
        );
        let exported = library.audio(&id).unwrap();
        std::fs::write(&file, &exported).unwrap();
        let reimported = import_recording(&file).unwrap();
        assert!(exported == reimported);
        let another = library.save_voice("export", &reimported).unwrap();
        assert_ne!(another, id);
        let Voice::Generated(another) = library.voice_record(&another).unwrap() else {
            panic!("metadata lost");
        };
        assert_eq!(
            serde_json::to_value(another.conditions).unwrap(),
            serde_json::to_value(take.conditions).unwrap()
        );
        assert_eq!(
            std::fs::read_dir(library.root().join("voices"))
                .unwrap()
                .count(),
            2
        );
    }
}
