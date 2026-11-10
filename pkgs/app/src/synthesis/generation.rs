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
