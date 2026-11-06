use irodoru_engine::{Engine, EngineConfig, SynthesisRequest};

#[test]
#[ignore = "requires anime + codec; set IRODORU_TEST_MODELS and run --release"]
fn embedded_anime_reuses_and_clears_features_without_changing_wav() -> anyhow::Result<()> {
    let config = EngineConfig::anime(std::env::var("IRODORU_TEST_MODELS")?.into());
    let mut engine = Engine::load(&config)?;
    let generate = |engine: &mut Engine| -> anyhow::Result<Vec<u8>> {
        let job = SynthesisRequest {
            text: "こんにちは。".into(),
            caption: Some("落ち着いた声".into()),
            reference_audio: Some(include_bytes!("../../../tests/fixtures/test.wav").to_vec()),
            seed: Some(42),
            num_steps: Some(1),
            seconds: Some(0.5),
            ..Default::default()
        }
        .try_into()?;
        let (conditions, output) = engine.synthesize(job)?;
        assert_eq!(conditions.model, "anime");
        assert!(!output.audio.samples.is_empty());
        conditions.to_wav(&output.audio)
    };
    let cold = generate(&mut engine)?;
    assert_eq!(cold, generate(&mut engine)?);
    engine.clear_cache();
    assert_eq!(cold, generate(&mut engine)?);
    Ok(())
}

#[test]
fn missing_model_fails_before_loading_runtime() {
    let config = EngineConfig::anime(std::env::temp_dir().join("irodoru-no-such-model"));
    let error = Engine::load(&config).err().expect("must fail");
    assert!(error.to_string().contains("no model found in"));
}
