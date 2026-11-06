use crate::{
    domain::TakeRequest,
    domain::{Take, new_id},
    library::Library,
};
use anyhow::{Context, Result, bail};
use irodoru_engine::{GenerationConditions, SynthOutput, SynthesisRequest, TTSJob};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub struct PreparedTake {
    job: TTSJob,
    take: Take,
}

pub struct PreparedBatch {
    id: String,
    takes: Vec<Option<PreparedTake>>,
}
impl PreparedBatch {
    pub fn prepare(
        active: &mut Option<Self>,
        library: &Library,
        requests: Vec<TakeRequest>,
    ) -> Result<String> {
        if active.is_some() {
            bail!("生成は既に実行中です");
        }
        let takes = prepare(library, requests)?.into_iter().map(Some).collect();
        let id = new_id();
        *active = Some(Self {
            id: id.clone(),
            takes,
        });
        Ok(id)
    }
    pub fn take(&mut self, id: &str, index: usize) -> Result<PreparedTake> {
        if self.id != id {
            bail!("生成リクエストが見つかりません");
        }
        self.takes
            .get_mut(index)
            .and_then(Option::take)
            .context("生成リクエストが見つかりません")
    }
    pub fn finish(active: &mut Option<Self>, id: &str) {
        if active.as_ref().is_some_and(|batch| batch.id == id) {
            *active = None;
        }
    }
}

pub fn prepare(library: &Library, requests: Vec<TakeRequest>) -> Result<Vec<PreparedTake>> {
    if requests.is_empty() || requests.len() > 50 {
        bail!("生成行数は1〜50行です");
    }
    requests
        .into_iter()
        .map(|request| {
            let character = library.character(&request.character_id)?;
            prepare_one(library, request, character)
        })
        .collect()
}

pub fn prepare_preview(
    library: &Library,
    request: crate::domain::SynthesisInput,
    description: String,
    voice: Option<String>,
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
    prepare_one(library, request, character)
}

fn prepare_one(
    library: &Library,
    req: TakeRequest,
    character: crate::domain::Character,
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
        reference_audio: character
            .voice
            .as_ref()
            .map(|voice| library.voice(voice))
            .transpose()?,
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
    take.model = conditions.model;
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
    fn batch_rejects_duplicates_and_stale_ids_and_recovers_after_failure() {
        let (_dir, library, request) = fixture();
        let mut active = None;
        let id = PreparedBatch::prepare(&mut active, &library, vec![request.clone()]).unwrap();
        assert!(PreparedBatch::prepare(&mut active, &library, vec![request.clone()]).is_err());
        let batch = active.as_mut().unwrap();
        assert!(batch.take("stale", 0).is_err());
        assert!(batch.take(&id, 1).is_err());
        let prepared = batch.take(&id, 0).unwrap();
        assert!(batch.take(&id, 0).is_err());
        assert!(synthesize(prepared, |_| bail!("inference failed")).is_err());
        PreparedBatch::finish(&mut active, "stale");
        assert!(active.is_some());
        PreparedBatch::finish(&mut active, &id);
        assert!(active.is_none());
        let next = PreparedBatch::prepare(&mut active, &library, vec![request]).unwrap();
        PreparedBatch::finish(&mut active, &id);
        assert!(active.as_mut().unwrap().take(&next, 0).is_ok());
        assert!(library.takes().unwrap().is_empty());
        PreparedBatch::finish(&mut active, &next);
        assert!(PreparedBatch::prepare(&mut active, &library, vec![]).is_err());
        assert!(active.is_none());
    }

    #[test]
    fn preview_uses_draft_and_keeps_history_separate() {
        let (dir, library, request) = fixture();
        let previews = Library::open(&dir.path().join("previews")).unwrap();
        let prepared = prepare_preview(
            &library,
            request.request,
            " 明るい声 ".into(),
            Some("reference".into()),
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
        let (_dir, library, request) = fixture();
        let character = request.character_id.clone();
        let prepared = prepare(&library, vec![request]).unwrap().remove(0);
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
    }
    #[test]
    fn invalid_request_and_failed_inference_do_not_publish_a_take() {
        let (_dir, library, mut request) = fixture();
        request.request.duration_scale = 0.0;
        assert!(prepare(&library, vec![request]).is_err());
        let (_dir, library, request) = fixture();
        let prepared = prepare(&library, vec![request]).unwrap().remove(0);
        assert!(synthesize(prepared, |_| anyhow::bail!("inference failed")).is_err());
        assert!(library.takes().unwrap().is_empty());
    }
}
