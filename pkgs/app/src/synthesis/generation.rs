use crate::{
    domain::TakeRequest,
    domain::{Take, new_id},
};
use anyhow::{Result, bail};
use irodoru_engine::{GenerationConditions, SynthOutput, SynthesisRequest, TTSJob};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub struct PreparedTake {
    job: TTSJob,
    take: Take,
}

impl PreparedTake {
    pub(crate) fn row_id(&self) -> &str {
        &self.take.row_id
    }
}

pub struct GenerationInput {
    pub request: TakeRequest,
    pub character: crate::domain::Character,
    pub reference_audio: Option<Vec<u8>>,
}

pub fn prepare(inputs: Vec<GenerationInput>) -> Result<Vec<PreparedTake>> {
    if inputs.is_empty() {
        bail!("生成するセリフがありません");
    }
    inputs
        .into_iter()
        .map(|input| prepare_one(input.request, input.character, input.reference_audio))
        .collect()
}

pub fn prepare_preview(
    request: crate::domain::SynthesisInput,
    description: String,
    voice: Option<String>,
    reference_audio: Option<Vec<u8>>,
) -> Result<PreparedTake> {
    if description.chars().count() > 2000 {
        bail!("プロンプトは2000文字以内です");
    }
    let character = crate::domain::Character {
        id: new_id(),
        name: "サンプル".into(),
        description,
        voice,
        color: crate::domain::DEFAULT_CHARACTER_COLOR.into(),
        sample_text: request.text.clone(),
        icon: None,
    };
    let request = TakeRequest {
        row_id: new_id(),
        character_id: character.id.clone(),
        additional_prompt: None,
        request,
    };
    prepare_one(request, character, reference_audio)
}

fn prepare_one(
    req: TakeRequest,
    character: crate::domain::Character,
    reference_audio: Option<Vec<u8>>,
) -> Result<PreparedTake> {
    if req.row_id.is_empty()
        || req.row_id.len() > 64
        || req.request.text.trim().is_empty()
        || req.request.text.chars().count() > 1000
    {
        bail!("セリフまたは行 ID が不正です");
    }
    let extra = req.additional_prompt.unwrap_or_default().trim().to_owned();
    if extra.chars().count() > 1000 {
        bail!("追加プロンプトは1000文字以内です");
    }
    let description = [character.description.trim(), extra.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let job = SynthesisRequest {
        text: req.request.text.clone(),
        caption: Some(description.clone()),
        reference_audio,
        seed: req.request.seed,
        num_steps: req.request.num_steps,
        duration_scale: Some(req.request.duration_scale),
        cfg_scale_text: Some(req.request.cfg_scale_text),
        cfg_scale_speaker: Some(req.request.cfg_scale_speaker),
        cfg_scale_caption: Some(req.request.cfg_scale_caption),
        ..Default::default()
    }
    .try_into()?;
    Ok(PreparedTake {
        job,
        take: Take {
            filename: None,
            conditions: None,
            id: new_id(),
            row_id: req.row_id,
            character_id: Some(character.id),
            character_name: character.name,
            description,
            additional_prompt: extra,
            voice: character.voice,
            text: req.request.text,
            model: String::new(),
            seed: 0,
            num_steps: 0,
            elapsed_ms: 0,
            audio_seconds: 0.0,
            created_at: 0,
        },
    })
}

pub fn synthesize(
    prepared: PreparedTake,
    infer: impl FnOnce(TTSJob) -> Result<(GenerationConditions, SynthOutput)>,
) -> Result<(Take, Vec<u8>)> {
    let started = Instant::now();
    let (conditions, output) = infer(prepared.job)?;
    let wav = conditions.to_wav(&output.audio)?;
    let mut take = prepared.take;
    take.filename = Some(conditions.filename()?);
    take.model = conditions.model.clone();
    take.conditions = Some(conditions);
    take.seed = output.seed;
    take.num_steps = output.num_steps;
    take.elapsed_ms = started.elapsed().as_millis() as u64;
    take.audio_seconds = output.audio.seconds();
    take.created_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    Ok((take, wav))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Library;
    fn input(library: &Library, request: TakeRequest) -> GenerationInput {
        let character = library.character(&request.character_id).unwrap();
        let reference_audio = character.voice.as_ref().map(|v| library.voice(v).unwrap());
        GenerationInput {
            request,
            character,
            reference_audio,
        }
    }
    use crate::{domain::CharacterInput, domain::SynthesisInput};
    use std::io::Cursor;
    fn wav() -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        let mut writer = hound::WavWriter::new(
            &mut cursor,
            hound::WavSpec {
                channels: 1,
                sample_rate: 48000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..48000 {
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();
        cursor.into_inner()
    }
    fn fixture() -> (tempfile::TempDir, Library, TakeRequest) {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        library.save_voice("reference", &wav()).unwrap();
        let character = library
            .create_character(CharacterInput {
                icon: None,
                color: None,
                sample_text: None,
                name: "さくら".into(),
                description: "落ち着いた声".into(),
                voice: Some("reference".into()),
            })
            .unwrap();
        let req = TakeRequest {
            row_id: "row".into(),
            character_id: character.id,
            additional_prompt: Some(" 小声で ".into()),
            request: SynthesisInput {
                text: "こんにちは".into(),
                seed: Some(42),
                num_steps: None,
                duration_scale: 1.0,
                cfg_scale_text: 3.0,
                cfg_scale_speaker: 5.0,
                cfg_scale_caption: 3.0,
            },
        };
        (dir, library, req)
    }

    #[test]
    fn preparation_rejects_empty_or_partially_invalid_queues() {
        let (_dir, library, request) = fixture();
        assert!(prepare(vec![]).is_err());
        let mut invalid = request.clone();
        invalid.request.duration_scale = 0.0;
        assert!(
            prepare(vec![
                input(&library, request.clone()),
                input(&library, invalid)
            ])
            .is_err()
        );
        assert!(library.takes().unwrap().is_empty());
        assert_eq!(prepare(vec![input(&library, request)]).unwrap().len(), 1);
    }

    #[test]
    fn preview_uses_draft_and_keeps_history_separate() {
        let (dir, library, request) = fixture();
        let previews = Library::open(&dir.path().join("previews")).unwrap();
        let prepared = prepare_preview(
            request.request,
            " 明るい声 ".into(),
            Some("reference".into()),
            Some(library.voice("reference").unwrap()),
        )
        .unwrap();
        let (take, wav) = synthesize(prepared, |job| {
            assert_eq!(job.params.caption.as_deref(), Some("明るい声"));
            assert!(job.reference_audio.is_some());
            Ok((
                GenerationConditions::from_params("anime".into(), &job.params, 42, 1, 48000),
                SynthOutput {
                    audio: irodoru_engine::Audio::new(vec![0.0; 24000], 48000),
                    seed: 42,
                    num_steps: 1,
                    timings: vec![],
                },
            ))
        })
        .unwrap();
        assert_eq!(take.audio_seconds, 0.5);
        previews.save_take(&take, &wav).unwrap();
        assert!(library.takes().unwrap().is_empty());
        assert_eq!(previews.audio(&take.id).unwrap(), wav);
        previews.delete_take(&take.id).unwrap();
        assert!(previews.takes().unwrap().is_empty());
        assert_eq!(library.characters().unwrap().len(), 1);
    }

    #[test]
    fn snapshot_survives_character_removal_and_saves_resolved_output() {
        let (dir, library, mut request) = fixture();
        request.request.duration_scale = 1.25;
        request.request.cfg_scale_text = 2.0;
        request.request.cfg_scale_speaker = 4.0;
        request.request.cfg_scale_caption = 6.0;
        let mut row = crate::workspace::editor::new_row(String::new(), Default::default());
        row.id = request.row_id.clone();
        library
            .save_editor(&crate::domain::EditorState {
                rows: vec![row],
                ..Default::default()
            })
            .unwrap();
        let character = request.character_id.clone();
        let prepared = prepare(vec![input(&library, request)]).unwrap().remove(0);
        library.delete_character(&character).unwrap();
        let (take, bytes) = synthesize(prepared, |job| {
            assert_eq!(job.params.caption.as_deref(), Some("落ち着いた声\n小声で"));
            assert_eq!(job.reference_audio.as_deref(), Some(wav().as_slice()));
            let conditions =
                GenerationConditions::from_params("anime".into(), &job.params, 42, 40, 48000);
            Ok((
                conditions,
                SynthOutput {
                    audio: irodoru_engine::Audio::new(vec![0.0; 48000], 48000),
                    seed: 42,
                    num_steps: 40,
                    timings: vec![],
                },
            ))
        })
        .unwrap();
        assert_eq!(take.character_name, "さくら");
        assert_eq!(take.model, "anime");
        assert_eq!(take.num_steps, 40);
        assert_eq!(take.audio_seconds, 1.0);
        assert!(take.filename.as_ref().unwrap().ends_with(".wav"));
        library.save_take(&take, &bytes).unwrap();
        assert_eq!(library.audio(&take.id).unwrap(), bytes);
        let library = Library::open(dir.path()).unwrap();
        let saved = library.takes().unwrap().remove(0);
        let conditions = saved.conditions.as_ref().unwrap();
        assert_eq!(conditions.duration_scale, 1.25);
        assert_eq!(conditions.cfg_scale_caption, 6.0);
        assert_eq!(conditions.caption.as_deref(), Some("落ち着いた声\n小声で"));
        let mut editor = library.editor().unwrap();
        editor.rows[0].text = "書き換えたセリフ".into();
        editor.rows[0].settings = Some(Default::default());
        editor.rows[0].prompt = None;
        library.save_editor(&editor).unwrap();
        library.select_take(&take.row_id, Some(&take.id)).unwrap();
        let editor = library.editor().unwrap();
        let row = &editor.rows[0];
        let settings = row.settings.as_ref().unwrap();
        assert_eq!(row.text, "こんにちは");
        assert_eq!(row.prompt.as_deref(), Some("小声で"));
        assert_eq!(settings.seed, "42");
        assert_eq!(settings.num_steps, "40");
        assert_eq!(settings.duration_scale, 1.25);
        assert_eq!(
            (
                settings.cfg_text,
                settings.cfg_speaker,
                settings.cfg_caption
            ),
            (2.0, 4.0, 6.0)
        );
        assert_eq!(
            serde_json::to_value(&library.takes().unwrap()[0].conditions).unwrap(),
            serde_json::to_value(&saved.conditions).unwrap()
        );
    }
}
