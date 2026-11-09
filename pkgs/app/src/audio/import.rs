use anyhow::{Context, Result, bail};
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction, audioadapter_buffers::direct::InterleavedSlice,
};
use std::{io::Cursor, path::Path};
use symphonia::core::{
    codecs::audio::AudioDecoderOptions,
    formats::{FormatOptions, TrackType, probe::Hint},
    io::MediaSourceStream,
    meta::MetadataOptions,
};

pub const AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "flac", "ogg", "oga", "aac", "m4a", "mp4"];
pub fn import_audio(path: &Path) -> Result<Vec<u8>> {
    let source = MediaSourceStream::new(Box::new(std::fs::File::open(path)?), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe().probe(
        &hint,
        source,
        FormatOptions::default(),
        MetadataOptions::default(),
    )?;
    let track = format
        .default_track(TrackType::Audio)
        .context("音声トラックがありません")?;
    let id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .context("音声トラックの形式を読み込めません")?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())?;
    let mut mono = Vec::new();
    let mut rate = 0;
    let mut samples = Vec::<f32>::new();
    while let Some(packet) = format.next_packet()? {
        if packet.track_id != id {
            continue;
        }
        let decoded = decoder.decode(&packet)?;
        // Some compressed packets contain only decoder priming information.
        if decoded.frames() == 0 {
            continue;
        }
        let spec = decoded.spec();
        if rate != 0 && rate != spec.rate() {
            bail!("途中でサンプルレートが変わる音声は読み込めません");
        }
        rate = spec.rate();
        if rate == 0 || spec.channels().count() == 0 {
            bail!("不正な音声です");
        }
        samples.resize(decoded.samples_interleaved(), 0.0);
        decoded.copy_to_slice_interleaved(&mut samples);
        for frame in samples.chunks(spec.channels().count()) {
            let sample = frame.iter().sum::<f32>() / frame.len() as f32;
            if !sample.is_finite() {
                bail!("不正な音声サンプルです");
            }
            mono.push(sample);
            if mono.len() >= rate as usize * 60 {
                break;
            }
        }
        if mono.len() >= rate as usize * 60 {
            break;
        }
    }
    normalize_audio(mono, rate)
}
pub fn normalize_audio(mut mono: Vec<f32>, rate: u32) -> Result<Vec<u8>> {
    if rate == 0 || mono.len() < rate as usize {
        bail!("1秒以上の音声が必要です");
    }
    mono.truncate(rate as usize * 60);
    if mono.iter().any(|v| !v.is_finite()) {
        bail!("不正な音声サンプルです");
    }
    let samples = if rate == 48000 {
        mono
    } else {
        let count = (mono.len() as u64 * 48000 / rate as u64) as usize;
        let mut resampler = Async::<f32>::new_sinc(
            48000.0 / rate as f64,
            1.0,
            &SincInterpolationParameters {
                sinc_len: 128,
                f_cutoff: Some(0.95),
                interpolation: SincInterpolationType::Linear,
                oversampling_factor: 128,
                window: WindowFunction::BlackmanHarris2,
            },
            1024,
            1,
            FixedAsync::Input,
        )?;
        let input = InterleavedSlice::new(&mono, 1, mono.len())?;
        let mut output = resampler.process_all(&input, mono.len(), None)?.take_data();
        // Preserve the existing duration rounding; process_all rounds up.
        output.truncate(count);
        output
    };
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut buffer,
            hound::WavSpec {
                channels: 1,
                sample_rate: 48000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )?;
        for sample in samples {
            writer.write_sample((sample.clamp(-1.0, 1.0) * 32767.0).round() as i16)?;
        }
        writer.finalize()?;
    }
    Ok(buffer.into_inner())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duration_and_resampling_boundaries() {
        assert!(normalize_audio(vec![0.0; 7999], 8000).is_err());
        for rate in [8000, 44100, 48000] {
            let wav = normalize_audio(vec![0.25; rate as usize], rate).unwrap();
            let reader = hound::WavReader::new(Cursor::new(wav)).unwrap();
            assert_eq!(reader.duration(), 48000);
            assert_eq!(reader.spec().channels, 1);
            assert_eq!(reader.spec().bits_per_sample, 16);
        }
        let wav = normalize_audio(vec![0.0; 48000 * 61], 48000).unwrap();
        assert_eq!(
            hound::WavReader::new(Cursor::new(wav)).unwrap().duration(),
            48000 * 60
        );
    }
    #[test]
    fn resampling_preserves_duration_and_trims_delay() {
        for rate in [8000, 44100, 96000] {
            let frames = rate as usize + 137;
            let ramp = (0..frames)
                .map(|i| 0.1 + 0.5 * i as f32 / frames as f32)
                .collect();
            let wav = normalize_audio(ramp, rate).unwrap();
            let mut reader = hound::WavReader::new(Cursor::new(wav)).unwrap();
            let samples: Vec<i16> = reader.samples().map(Result::unwrap).collect();
            assert_eq!(samples.len(), frames * 48000 / rate as usize);
            // Check alignment away from the sinc filter's edge ringing.
            for index in [1024, samples.len() - 1024] {
                let expected =
                    (0.1 + 0.5 * index as f64 * rate as f64 / (48000.0 * frames as f64)) * 32767.0;
                assert!(
                    (samples[index] as f64 - expected).abs() < 5.0,
                    "rate={rate}, index={index}, actual={}, expected={expected}",
                    samples[index]
                );
            }
        }
    }
}
