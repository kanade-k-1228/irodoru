use super::*;
use crate::engine::Voices;
use crate::model::DevicePreference;
use crate::output::GenerationConditions;
use crate::{SynthesisRequest, TTSJob};
use serde_json::json;

fn close(label: &str, a: &[f32], b: &[f32], atol: f32, rtol: f32) {
    assert_eq!(a.len(), b.len());
    for (i, (&a, &b)) in a.iter().zip(b).enumerate() {
        assert!(
            a.is_finite() && b.is_finite() && (a - b).abs() <= atol + rtol * a.abs(),
            "{label}: mismatch at {i}: {a} versus {b}"
        );
    }
}
fn condition_eq(label: &str, a: &Condition, b: &Condition) {
    assert_eq!(a.state.shape(), b.state.shape());
    assert_eq!(a.mask, b.mask);
    close(
        label,
        a.state.as_slice().unwrap(),
        b.state.as_slice().unwrap(),
        1e-5,
        1e-4,
    );
}
fn read_latent(path: &Path) -> Vec<f32> {
    std::fs::read(path.join("latent.f32"))
        .unwrap()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|x| f32::from_le_bytes(*x))
        .collect()
}

#[test]
#[ignore = "requires exported models; run release with IRODORI_FEATURE_TEST_MODELS and IRODORI_DEBUG_DUMP"]
fn production_feature_cache_parity() -> Result<()> {
    ensure!(!cfg!(debug_assertions), "run --release");
    let root = std::path::PathBuf::from(std::env::var("IRODORI_FEATURE_TEST_MODELS")?);
    let dump = std::path::PathBuf::from(std::env::var("IRODORI_DEBUG_DUMP")?);
    let device: DevicePreference = std::env::var("IRODORI_FEATURE_TEST_DEVICE")
        .unwrap_or_else(|_| "cpu".into())
        .parse()?;
    let name = std::env::var("IRODORI_FEATURE_TEST_MODEL").unwrap_or_else(|_| "mf".into());
    let lib = std::env::var("IRODORI_OPENVINO_LIBRARY")
        .ok()
        .map(std::path::PathBuf::from);
    let runtime = Runtime::discover(2, device, lib.as_deref())?;
    let mut codec = Codec::load(&root.join("codec"), &runtime)?;
    let mut model = TTSModel::load(&root.join(&name), &runtime)?;
    let wav =
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/test.wav"))?;
    if device == DevicePreference::Gpu && std::env::var_os("IRODORI_FEATURE_TEST_WARMUP").is_some()
    {
        for warmup in 0..10 {
            let latent =
                codec.encode_reference(Audio::from_wav(&wav)?, model.manifest.max_ref_seconds)?;
            let req: SynthesisRequest =
                serde_json::from_value(json!({"text":"こんにちは。今日はいい天気ですね。",
                "caption":"落ち着いた自然な声で、明瞭に話す。声の設定0。",
                "seconds":1.0,"seed":19,"num_steps":4,"trim_tail":false}))?;
            let mut p = TTSJob::try_from(req)
                .map_err(|e| anyhow::anyhow!("{e:?}"))?
                .params;
            p.speaker = Some(model.encode_speaker(&latent, &codec)?);
            model.caption_cache.clear();
            let result = model.synthesize(&mut codec, &p)?;
            println!(
                "uncached GPU warmup {warmup}: {}",
                crate::output::sha256(&result.audio.to_wav()?)
            );
        }
        model.caption_cache.clear();
    }
    let mut voices = Voices::default();
    let speaker = voices
        .speaker(&wav, &mut model, &mut codec)
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let before = model.encode_counts;
    condition_eq(
        "speaker cache hit",
        &speaker,
        &voices
            .speaker(&wav, &mut model, &mut codec)
            .map_err(|e| anyhow::anyhow!("{e:?}"))?,
    );
    assert_eq!(model.encode_counts, before);
    let original_latent =
        codec.encode_reference(Audio::from_wav(&wav)?, model.manifest.max_ref_seconds)?;
    let latent = codec.encode_reference(Audio::from_wav(&wav)?, model.manifest.max_ref_seconds)?;
    close(
        "codec re-encode",
        original_latent.as_slice().unwrap(),
        latent.as_slice().unwrap(),
        1e-5,
        1e-4,
    );
    condition_eq(
        "speaker re-encode with identical latent",
        &speaker,
        &model.encode_speaker(&original_latent, &codec)?,
    );
    condition_eq(
        "speaker re-encode with new latent",
        &speaker,
        &model.encode_speaker(&latent, &codec)?,
    );

    let captions: Vec<_> = (0..10)
        .map(|i| format!("落ち着いた自然な声で、明瞭に話す。声の設定{i}。"))
        .collect();
    for caption in &captions {
        let expected = model.encode_text(caption, true)?;
        condition_eq(
            &format!("caption {caption}"),
            &expected,
            &model.caption_condition(caption)?,
        );
    }
    let before = model.encode_counts;
    for _ in 0..2 {
        for caption in &captions {
            let mut copy = model.caption_condition(&format!(" {caption} \n"))?;
            copy.state.fill(0.0);
            assert!(
                model
                    .caption_condition(caption)?
                    .state
                    .iter()
                    .any(|&v| v != 0.0)
            );
        }
    }
    assert_eq!(model.encode_counts, before);
    assert_eq!(model.caption_cache.len(), 10);
    assert!(!model.caption_condition(" \n ")?.is_present());
    assert_eq!(model.encode_counts, before);

    let req: SynthesisRequest =
        serde_json::from_value(json!({"text":"こんにちは。今日はいい天気ですね。",
        "caption":captions[0],"seconds":1.0,"seed":19,"num_steps":4,"trim_tail":false}))?;
    let mut params = TTSJob::try_from(req)
        .map_err(|e| anyhow::anyhow!("{e:?}"))?
        .params;
    params.speaker = Some(speaker);
    model.caption_cache.clear();
    let before = model.encode_counts;
    let cold = model.synthesize(&mut codec, &params)?;
    let cold_latent = read_latent(&dump);
    let warm = model.synthesize(&mut codec, &params)?;
    let warm_latent = read_latent(&dump);
    assert_eq!(model.encode_counts[0], before[0] + 2);
    assert_eq!(model.encode_counts[1], before[1] + 1);
    close("cold/warm latent", &cold_latent, &warm_latent, 1e-5, 1e-4);
    close(
        "cold/warm waveform",
        &cold.audio.samples,
        &warm.audio.samples,
        1e-4,
        1e-3,
    );
    let conditions = |out: &SynthOutput| {
        let mut c = GenerationConditions::from_params(
            name.clone(),
            &params,
            out.seed,
            out.num_steps,
            out.audio.sample_rate,
        );
        c.voice = Some(crate::output::sha256(&wav));
        c.reference_sha256 = Some(crate::output::sha256(&wav));
        c
    };
    assert_eq!(conditions(&cold).filename()?, conditions(&warm).filename()?);
    let cold_wav = conditions(&cold).to_wav(&cold.audio)?;
    let warm_wav = conditions(&warm).to_wav(&warm.audio)?;
    let cold_hash = crate::output::sha256(&cold_wav);
    let warm_hash = crate::output::sha256(&warm_wav);
    std::fs::write(
        dump.join("parity.json"),
        serde_json::to_vec_pretty(&json!({
            "model": name, "device": device.to_string(),
            "features_latent_waveform_within_tolerance": true,
            "generation_conditions_identical": true,
            "wav_identical": cold_wav == warm_wav,
            "cold_wav_sha256": cold_hash, "warm_wav_sha256": warm_hash,
            "latent_max_abs_difference": cold_latent.iter().zip(&warm_latent)
                .map(|(a, b)| (a - b).abs()).fold(0.0_f32, f32::max),
            "waveform_max_abs_difference": cold.audio.samples.iter().zip(&warm.audio.samples)
                .map(|(a, b)| (a - b).abs()).fold(0.0_f32, f32::max),
        }))?,
    )?;
    let counts = model.encode_counts;
    params.text = "窓を開けると、涼しい風が入ってきました。".into();
    model.synthesize(&mut codec, &params)?;
    assert_eq!(model.encode_counts[0], counts[0] + 1);
    assert_eq!(model.encode_counts[1], counts[1]);
    params.caption = Some(captions[1].clone());
    model.synthesize(&mut codec, &params)?;
    assert_eq!(model.encode_counts[1], counts[1] + 1);
    assert_eq!(cold_hash, warm_hash, "metadata-bearing WAV differs");
    println!(
        "production feature cache parity passed: model={name} device={device}; cold={:?} warm={:?}",
        cold.timings, warm.timings
    );
    Ok(())
}
