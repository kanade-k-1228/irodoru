use anyhow::{Context, Result, bail};
use std::io::Cursor;
pub fn validate_wav(bytes: &[u8]) -> Result<f64> {
    let mut reader = hound::WavReader::new(Cursor::new(bytes)).context("invalid WAV")?;
    let spec = reader.spec();
    if spec.channels == 0 || spec.sample_rate == 0 || reader.duration() == 0 {
        bail!("empty WAV");
    }
    let duration = reader.duration() as f64 / spec.sample_rate as f64;
    match spec.sample_format {
        hound::SampleFormat::Float => {
            for sample in reader.samples::<f32>() {
                if !sample?.is_finite() {
                    bail!("non-finite WAV sample");
                }
            }
        }
        hound::SampleFormat::Int => {
            for sample in reader.samples::<i32>() {
                sample?;
            }
        }
    }
    Ok(duration)
}
