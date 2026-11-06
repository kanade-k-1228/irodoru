use irodoru_desktop::{
    generation::{self, PreparedBatch},
    inference::Inference,
    library::{CharacterInput, Library},
    types::{InferenceStatus, SynthesisInput, TakeRequest},
};
use irodoru_engine::{EngineConfig, SynthesisRequest};
use std::time::{Duration, Instant};

#[test]
#[ignore = "requires anime + codec; set IRODORU_TEST_MODELS and run --release"]
fn application_worker_generates_takes_and_previews_and_recovers() -> anyhow::Result<()> {
    let config = EngineConfig::anime(std::env::var("IRODORU_TEST_MODELS")?.into());
    let inference = Inference::start_with(move || Ok(config))?;
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        match inference.status() {
            InferenceStatus::Ready => break,
            InferenceStatus::Error { error } => anyhow::bail!("{error}"),
            _ => {
                anyhow::ensure!(Instant::now() < deadline, "model loading timed out");
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    let invalid = SynthesisRequest {
        text: "こんにちは。".into(),
        cfg_scale_text: Some(f32::NAN),
        ..Default::default()
    }
    .try_into()?;
    assert!(inference.synthesize(invalid).is_err());
    assert!(matches!(inference.status(), InferenceStatus::Ready));

    let directory = tempfile::tempdir()?;
    let library = Library::open(directory.path())?;
    let previews = Library::open(&directory.path().join("previews"))?;
    let character = library.create_character(CharacterInput {
        name: "テスト".into(),
        description: "落ち着いた声".into(),
        voice: None,
        icon: None,
        color: None,
        sample_text: None,
    })?;
    let input = SynthesisInput {
        text: "こんにちは。".into(),
        seed: Some(42),
        num_steps: Some(1),
        duration_scale: 1.0,
        cfg_scale_text: 3.0,
        cfg_scale_speaker: 5.0,
        cfg_scale_caption: 3.0,
    };
    let request = TakeRequest {
        row_id: "row".into(),
        character_id: character.id,
        additional_prompt: None,
        request: input.clone(),
    };
    let mut batch = None;
    let id = PreparedBatch::prepare(&mut batch, &library, vec![request])?;
    let prepared = batch.as_mut().unwrap().take(&id, 0)?;
    let (take, wav) = generation::synthesize(prepared, |job| inference.synthesize(job))?;
    library.save_take(&take, &wav)?;
    PreparedBatch::finish(&mut batch, &id);
    assert_eq!(take.model, "anime");
    assert!(take.audio_seconds > 0.0);
    assert!(matches!(inference.status(), InferenceStatus::Ready));

    let preview = generation::prepare_preview(&library, input, character.description, None)?;
    let (preview, preview_wav) = generation::synthesize(preview, |job| inference.synthesize(job))?;
    assert_eq!(
        irodoru_engine::output::sha256(&wav),
        irodoru_engine::output::sha256(&preview_wav)
    );
    previews.save_take(&preview, &preview_wav)?;
    assert_eq!(library.takes()?.len(), 1);
    assert_eq!(previews.takes()?.len(), 1);
    assert_eq!(Library::open(directory.path())?.audio(&take.id)?, wav);
    println!(
        "anime take and cached preview match: {:.3}s",
        take.audio_seconds
    );
    Ok(())
}
