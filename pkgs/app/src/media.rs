use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::{io::Cursor, path::Path};
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, errors::Error, formats::FormatOptions,
    io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};

pub const AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "flac", "ogg", "oga", "aac", "m4a", "mp4"];
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "svg",
];
pub fn import_audio(path: &Path) -> Result<Vec<u8>> {
    let source = MediaSourceStream::new(Box::new(std::fs::File::open(path)?), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .format(
            &hint,
            source,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?
        .format;
    let track = format.default_track().context("音声トラックがありません")?;
    let id = track.id;
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let mut mono = Vec::new();
    let mut rate = 0;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        };
        if packet.track_id() != id {
            continue;
        }
        let decoded = decoder.decode(&packet)?;
        // Some compressed packets contain only decoder priming information.
        // Symphonia cannot interleave an empty multichannel buffer.
        if decoded.frames() == 0 {
            continue;
        }
        let spec = *decoded.spec();
        if rate != 0 && rate != spec.rate {
            bail!("途中でサンプルレートが変わる音声は読み込めません");
        }
        rate = spec.rate;
        if rate == 0 || spec.channels.count() == 0 {
            bail!("不正な音声です");
        }
        let mut samples = SampleBuffer::<f32>::new(decoded.frames() as u64, spec);
        samples.copy_interleaved_ref(decoded);
        for frame in samples.samples().chunks(spec.channels.count()) {
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
        let mut resampler = SincFixedIn::<f32>::new(
            48000.0 / rate as f64,
            1.0,
            SincInterpolationParameters {
                sinc_len: 128,
                f_cutoff: 0.95,
                interpolation: SincInterpolationType::Linear,
                oversampling_factor: 128,
                window: WindowFunction::BlackmanHarris2,
            },
            1024,
            1,
        )?;
        let delay = resampler.output_delay();
        let mut output = Vec::with_capacity(count + delay + 2048);
        for chunk in mono.chunks(1024) {
            let mut input = vec![0.0; 1024];
            input[..chunk.len()].copy_from_slice(chunk);
            output.extend(resampler.process(&[input], None)?.remove(0));
        }
        while output.len() < count + delay {
            output.extend(resampler.process(&[vec![0.0; 1024]], None)?.remove(0));
        }
        output[delay..delay + count].to_vec()
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
pub fn import_icon(path: &Path) -> Result<String> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > 10 * 1024 * 1024 {
        bail!("10 MB以下の画像を選択してください");
    }
    let bytes = std::fs::read(path)?;
    let image = if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
    {
        let tree = resvg::usvg::Tree::from_data(&bytes, &resvg::usvg::Options::default())?;
        let size = tree.size();
        let scale = 128.0 / size.width().min(size.height());
        let mut pixmap = resvg::tiny_skia::Pixmap::new(128, 128).context("画像を読み込めません")?;
        let transform = resvg::tiny_skia::Transform::from_row(
            scale,
            0.0,
            0.0,
            scale,
            (128.0 - size.width() * scale) / 2.0,
            (128.0 - size.height() * scale) / 2.0,
        );
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        return Ok(format!(
            "data:image/png;base64,{}",
            STANDARD.encode(pixmap.encode_png()?)
        ));
    } else {
        let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(128 * 1024 * 1024);
        limits.max_image_width = Some(16384);
        limits.max_image_height = Some(16384);
        reader.limits(limits);
        reader.decode()?
    };
    let side = image.width().min(image.height());
    if side == 0 {
        bail!("画像を読み込めません");
    }
    let image = image
        .crop_imm(
            (image.width() - side) / 2,
            (image.height() - side) / 2,
            side,
            side,
        )
        .resize_exact(128, 128, image::imageops::FilterType::Lanczos3);
    let mut png = Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png)?;
    Ok(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(png.into_inner())
    ))
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
    fn image_crop_and_svg_are_png() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("icon.png");
        image::RgbImage::new(320, 180).save(&p).unwrap();
        for path in [p, dir.path().join("icon.svg")] {
            if path.extension().unwrap() == "svg" {
                std::fs::write(&path,r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><rect width="200" height="100" fill="red"/></svg>"#).unwrap();
            }
            let data = import_icon(&path).unwrap();
            let image =
                image::load_from_memory(&STANDARD.decode(data.split_once(',').unwrap().1).unwrap())
                    .unwrap();
            assert_eq!((image.width(), image.height()), (128, 128));
        }
    }
}
