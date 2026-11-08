use irodoru_engine::{Engine, EngineConfig};

#[test]
fn missing_model_fails_before_loading_runtime() {
    let config = EngineConfig::anime(std::env::temp_dir().join("irodoru-no-such-model"));
    let error = Engine::load(&config).err().expect("must fail");
    assert!(error.to_string().contains("no model found in"));
}
