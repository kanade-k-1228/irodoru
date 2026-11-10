use crate::{
    domain::TakeRequest,
    domain::{Take, new_id},
};
use anyhow::{Result, bail};
use irodoru_engine::{GenerationConditions, SynthOutput, SynthesisRequest, TTSJob};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub struct PreparedTake {
    job: TTSJob,
    line_id: String,
    chara: crate::domain::Chara,
    additional_prompt: String,
}

impl PreparedTake {
    pub(crate) fn line_id(&self) -> &str {
        &self.line_id
    }
}

pub struct GenerationInput {
    pub request: TakeRequest,
    pub chara: crate::domain::Chara,
    pub reference_audio: Vec<Vec<u8>>,
}

pub fn prepare(inputs: Vec<GenerationInput>) -> Result<Vec<PreparedTake>> {
    if inputs.is_empty() {
        bail!("生成するセリフがありません");
    }
    inputs
        .into_iter()
        .map(|input| prepare_one(input.request, input.chara, input.reference_audio))
        .collect()
}

pub fn prepare_preview(
    request: crate::domain::SynthesisInput,
    description: String,
    voice_ids: Vec<String>,
    reference_audio: Vec<Vec<u8>>,
) -> Result<PreparedTake> {
    if description.chars().count() > 2000 {
        bail!("プロンプトは2000文字以内です");
    }
    let chara = crate::domain::Chara {
        id: new_id(),
        name: "サンプル".into(),
        description,
        voice_ids,
        color: crate::domain::DEFAULT_CHARA_COLOR.into(),
        sample_text: request.text.clone(),
        icon: None,
    };
    let request = TakeRequest {
        line_id: new_id(),
        chara_id: chara.id.clone(),
        additional_prompt: None,
        request,
    };
    prepare_one(request, chara, reference_audio)
}

fn prepare_one(
    req: TakeRequest,
    chara: crate::domain::Chara,
    reference_audio: Vec<Vec<u8>>,
) -> Result<PreparedTake> {
    if req.line_id.is_empty()
        || req.line_id.len() > 64
        || req.request.text.trim().is_empty()
        || req.request.text.chars().count() > 1000
    {
        bail!("セリフまたは行 ID が不正です");
    }
    let extra = req.additional_prompt.unwrap_or_default().trim().to_owned();
    if extra.chars().count() > 1000 {
        bail!("追加プロンプトは1000文字以内です");
    }
    let description = [chara.description.trim(), extra.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let job = SynthesisRequest {
        text: req.request.text,
        caption: Some(description),
        reference_audios: reference_audio,
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
        line_id: req.line_id,
        chara,
        additional_prompt: extra,
    })
}

pub fn synthesize(
    prepared: PreparedTake,
    infer: impl FnOnce(TTSJob) -> Result<(GenerationConditions, SynthOutput)>,
) -> Result<(Take, Vec<u8>)> {
    let started = Instant::now();
    let (conditions, output) = infer(prepared.job)?;
    let wav = output.audio.to_wav()?;
    let take = Take {
        id: new_id(),
        line_id: prepared.line_id,
        chara_id: Some(prepared.chara.id),
        chara_name: prepared.chara.name,
        voice_ids: prepared.chara.voice_ids,
        additional_prompt: prepared.additional_prompt,
        conditions,
        elapsed_ms: started.elapsed().as_millis() as u64,
        audio_seconds: output.audio.seconds(),
        created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64,
    };
    Ok((take, wav))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Library;
    fn input(library: &Library, request: TakeRequest) -> GenerationInput {
        let chara = library.chara(&request.chara_id).unwrap();
        let reference_audio = library.reference_audio(&chara.voice_ids).unwrap();
        GenerationInput {
            request,
            chara,
            reference_audio,
        }
    }
    use crate::{domain::CharaInput, domain::SynthesisInput};
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
        let voice = library.save_voice("reference", &wav()).unwrap();
        let chara = library
            .create_chara(CharaInput {
                icon: None,
                color: None,
                sample_text: None,
                name: "さくら".into(),
                description: "落ち着いた声".into(),
                voice_ids: vec![voice],
            })
            .unwrap();
        let req = TakeRequest {
            line_id: "line".into(),
            chara_id: chara.id,
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
    fn preview_uses_chara_inputs_and_keeps_history_separate() {
        let (dir, library, request) = fixture();
        let previews = Library::open(&dir.path().join("previews")).unwrap();
        let prepared = prepare_preview(
            request.request,
            " 明るい声 ".into(),
            library.charas().unwrap()[0].voice_ids.clone(),
            library
                .reference_audio(&library.charas().unwrap()[0].voice_ids)
                .unwrap(),
        )
        .unwrap();
        let (take, wav) = synthesize(prepared, |job| {
            assert_eq!(job.params.caption.as_deref(), Some("明るい声"));
            assert!(!job.reference_audios.is_empty());
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
        assert_eq!(
            irodoru_engine::Audio::from_wav(&previews.audio(&take.id).unwrap())
                .unwrap()
                .samples,
            irodoru_engine::Audio::from_wav(&wav).unwrap().samples
        );
        previews.delete_take(&take.id).unwrap();
        assert!(previews.takes().unwrap().is_empty());
        assert_eq!(library.charas().unwrap().len(), 1);
    }

    #[test]
    fn snapshot_survives_chara_removal_and_saves_resolved_output() {
        let (dir, library, mut request) = fixture();
        request.request.duration_scale = 1.25;
        request.request.cfg_scale_text = 2.0;
        request.request.cfg_scale_speaker = 4.0;
        request.request.cfg_scale_caption = 6.0;
        let mut line = crate::workspace::editor::new_line(String::new(), Default::default());
        line.id = request.line_id.clone();
        library
            .save_editor(&crate::domain::EditorState {
                lines: vec![line],
                ..Default::default()
            })
            .unwrap();
        let chara = request.chara_id.clone();
        let reference_audio = library
            .reference_audio(&library.chara(&chara).unwrap().voice_ids)
            .unwrap();
        let prepared = prepare(vec![input(&library, request)]).unwrap().remove(0);
        library.delete_chara(&chara).unwrap();
        let (take, bytes) = synthesize(prepared, |job| {
            assert_eq!(job.params.caption.as_deref(), Some("落ち着いた声\n小声で"));
            assert!(job.reference_audios == reference_audio);
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
        assert_eq!(take.chara_name, "さくら");
        assert_eq!(take.conditions.model, "anime");
        assert_eq!(take.conditions.num_steps, 40);
        assert_eq!(take.audio_seconds, 1.0);
        assert!(take.conditions.filename().unwrap().ends_with(".wav"));
        library.save_take(&take, &bytes).unwrap();
        assert_eq!(
            irodoru_engine::Audio::from_wav(&library.audio(&take.id).unwrap())
                .unwrap()
                .samples,
            irodoru_engine::Audio::from_wav(&bytes).unwrap().samples
        );
        let library = Library::open(dir.path()).unwrap();
        let saved = library.takes().unwrap().remove(0);
        let conditions = &saved.conditions;
        assert_eq!(conditions.duration_scale, 1.25);
        assert_eq!(conditions.cfg_scale_caption, 6.0);
        assert_eq!(conditions.caption.as_deref(), Some("落ち着いた声\n小声で"));
        let mut editor = library.editor().unwrap();
        editor.lines[0].text = "書き換えたセリフ".into();
        editor.lines[0].settings = Some(Default::default());
        editor.lines[0].prompt = None;
        library.save_editor(&editor).unwrap();
        library.select_take(&take.line_id, Some(&take.id)).unwrap();
        let editor = library.editor().unwrap();
        let line = &editor.lines[0];
        let settings = line.settings.as_ref().unwrap();
        assert_eq!(line.text, "こんにちは");
        assert_eq!(line.prompt.as_deref(), Some("小声で"));
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
