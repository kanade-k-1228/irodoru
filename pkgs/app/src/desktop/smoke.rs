// Run the shipped executable without a display or audio device. This exercises its
// actual runtime linkage and executable-relative model paths after package extraction.
pub(super) fn package_smoke_test() -> anyhow::Result<()> {
    use irodoru_engine::{Engine, EngineConfig, SynthesisRequest};
    let path = crate::synthesis::resources::models()?;
    let mut engine = Engine::load(&EngineConfig::anime(path.clone()))?;
    let job = SynthesisRequest {
        text: "こんにちは。".into(),
        seed: Some(42),
        num_steps: Some(1),
        seconds: Some(0.5),
        ..Default::default()
    }
    .try_into()?;
    let (conditions, output) = engine.synthesize(job)?;
    let wav = conditions.to_wav(&output.audio)?;
    let reader = hound::WavReader::new(std::io::Cursor::new(wav))?;
    anyhow::ensure!(
        reader.duration() > 0,
        "Packaged model generated empty audio"
    );
    println!(
        "Packaged offline synthesis passed: {} frames, models at {}",
        reader.duration(),
        path.display()
    );
    Ok(())
}
