use std::io::Cursor;

use anyhow::{Result, bail, ensure};

pub struct Audio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl Audio {
    pub fn new(samples: Vec<f32>, sample_rate: u32) -> Self {
        Self {
            samples,
            sample_rate,
        }
    }

    pub fn from_wav(bytes: &[u8]) -> Result<Self> {
        let mut reader = hound::WavReader::new(bytes)?;
        let spec = reader.spec();
        ensure!(
            spec.channels > 0 && spec.sample_rate > 0,
            "invalid WAV channels or sample rate"
        );
        let ch = spec.channels as usize;
        let interleaved: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
            hound::SampleFormat::Int => {
                let scale = (1u64 << (spec.bits_per_sample - 1)) as f32;
                reader
                    .samples::<i32>()
                    .map(|s| s.map(|v| v as f32 / scale))
                    .collect::<Result<_, _>>()?
            }
        };
        ensure!(
            interleaved.iter().all(|s| s.is_finite()),
            "non-finite WAV sample"
        );
        let mono = interleaved
            .chunks(ch)
            .map(|frame| frame.iter().sum::<f32>() / ch as f32)
            .collect();
        Ok(Self::new(mono, spec.sample_rate))
    }

    pub fn to_wav(&self) -> Result<Vec<u8>> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: self.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut buf = Cursor::new(Vec::with_capacity(self.samples.len() * 2 + 64));
        {
            let mut w = hound::WavWriter::new(&mut buf, spec)?;
            for &s in &self.samples {
                w.write_sample((s.clamp(-1.0, 1.0) * 32767.0).round() as i16)?;
            }
            w.finalize()?;
        }
        Ok(buf.into_inner())
    }

    pub fn seconds(&self) -> f64 {
        self.samples.len() as f64 / self.sample_rate as f64
    }

    pub fn to_wav_with_comment(&self, comment: &str) -> Result<Vec<u8>> {
        let mut wav = self.to_wav()?;
        let size = u32::try_from(comment.len() + 1)?;
        let padded_size = size
            .checked_add(size % 2)
            .ok_or_else(|| anyhow::anyhow!("WAV comment too large"))?;
        let list_size = padded_size
            .checked_add(12)
            .ok_or_else(|| anyhow::anyhow!("WAV comment too large"))?;
        wav.extend_from_slice(b"LIST");
        wav.extend_from_slice(&list_size.to_le_bytes());
        wav.extend_from_slice(b"INFOICMT");
        wav.extend_from_slice(&size.to_le_bytes());
        wav.extend_from_slice(comment.as_bytes());
        wav.push(0);
        if size % 2 != 0 {
            wav.push(0);
        }
        let riff_size = u32::try_from(wav.len() - 8)?;
        wav[4..8].copy_from_slice(&riff_size.to_le_bytes());
        Ok(wav)
    }

    pub fn truncate_seconds(&mut self, seconds: f64) {
        let max = ((seconds * self.sample_rate as f64) as usize).max(1);
        self.samples.truncate(max);
    }

    pub fn resample(self, rate: u32) -> Self {
        let orig = self.sample_rate;
        if orig == rate || self.samples.is_empty() {
            return Self::new(self.samples, rate);
        }
        let wav = &self.samples;
        let g = gcd(orig as u64, rate as u64);
        let (of, nf) = ((orig as u64 / g) as i64, (rate as u64 / g) as i64);
        let lowpass = 6.0f64;
        let base = (of.min(nf) as f64) * 0.99;
        let width = (lowpass * of as f64 / base).ceil() as i64;
        let klen = (2 * width + of) as usize;
        let scale = base / of as f64;
        let mut kernels = vec![vec![0f32; klen]; nf as usize];
        for (j, kern) in kernels.iter_mut().enumerate() {
            for (k, v) in kern.iter_mut().enumerate() {
                let idx = (k as i64 - width) as f64 / of as f64;
                let mut t = (-(j as f64)) / nf as f64 + idx;
                t = (t * base).clamp(-lowpass, lowpass);
                let window = (t * std::f64::consts::PI / lowpass / 2.0).cos().powi(2);
                let tp = t * std::f64::consts::PI;
                let sinc = if tp == 0.0 { 1.0 } else { tp.sin() / tp };
                *v = (sinc * window * scale) as f32;
            }
        }
        let len = wav.len();
        let mut padded = vec![0f32; width as usize];
        padded.extend_from_slice(wav);
        padded.extend(std::iter::repeat_n(0f32, (width + of) as usize));
        let n_frames = (padded.len() - klen) / of as usize + 1;
        let target = ((nf as f64 * len as f64) / of as f64).ceil() as usize;
        let mut out = Vec::with_capacity(n_frames * nf as usize);
        'outer: for f in 0..n_frames {
            let seg = &padded[f * of as usize..f * of as usize + klen];
            for kern in &kernels {
                if out.len() >= target {
                    break 'outer;
                }
                out.push(seg.iter().zip(kern).map(|(a, b)| a * b).sum());
            }
        }
        out.truncate(target);
        Self::new(out, rate)
    }

    pub fn loudness(&self) -> Result<f64> {
        const SR: usize = 48_000;
        ensure!(
            self.sample_rate as usize == SR,
            "loudness needs 48 kHz audio, got {} Hz",
            self.sample_rate
        );
        let mut x: Vec<f64> = self.samples.iter().map(|&v| v as f64).collect();
        if x.len() < SR / 2 {
            x.resize(SR / 2, 0.0);
        }
        let y = biquad(
            &x,
            [1.5351828863637502, -2.691804030199196, 1.198426263333146],
            [1.0, -1.6906995865986896, 0.7325047060963897],
        );
        let y = biquad(
            &y,
            [0.9950442970178917, -1.9900885940357833, 0.9950442970178917],
            [1.0, -1.990076284018423, 0.9901009040531438],
        );
        let kernel = (0.4 * SR as f64) as usize;
        let stride = (0.4 * SR as f64 * 0.25) as usize;
        let n = y.len();
        let n_frames = ((n.max(kernel) - kernel) as f64 / stride as f64).ceil() as usize + 1;
        let z: Vec<f64> = (0..n_frames)
            .map(|f| {
                let start = f * stride;
                let end = (start + kernel).min(n);
                let s: f64 = if start < n {
                    y[start..end].iter().map(|v| v * v).sum()
                } else {
                    0.0
                };
                s / kernel as f64
            })
            .collect();
        let l: Vec<f64> = z.iter().map(|&zi| -0.691 + 10.0 * zi.log10()).collect();
        let gate = |thr_r: f64| -> f64 {
            let (mut sum, mut cnt) = (0.0, 0usize);
            for (zi, li) in z.iter().zip(&l) {
                if *li > -70.0 && *li > thr_r {
                    sum += zi;
                    cnt += 1;
                }
            }
            if cnt == 0 { f64::NAN } else { sum / cnt as f64 }
        };
        let abs_mean = gate(f64::NEG_INFINITY);
        let gamma_r = -0.691 + 10.0 * abs_mean.log10() - 10.0;
        let mut rel = gate(gamma_r);
        if rel.is_nan() {
            rel = 0.0;
        }
        let lufs = -0.691 + 10.0 * rel.log10();
        Ok(lufs.max(-70.0))
    }

    pub fn normalize_loudness(&mut self, target_db: f64) -> Result<()> {
        let gain = (10f64).powf((target_db - self.loudness()?) / 20.0) as f32;
        let mut peak = 0f32;
        for v in self.samples.iter_mut() {
            *v *= gain;
            peak = peak.max(v.abs());
        }
        if peak > 1.0 {
            for v in self.samples.iter_mut() {
                *v /= peak;
            }
        }
        Ok(())
    }

    pub fn reflect_pad_to_multiple(&mut self, n: usize) -> Result<()> {
        let len = self.samples.len();
        let pad = (n - len % n) % n;
        if pad == 0 {
            return Ok(());
        }
        if pad >= len {
            bail!("reference audio too short ({len} samples) for reflect padding");
        }
        let tail: Vec<f32> = (0..pad).map(|i| self.samples[len - 2 - i]).collect();
        self.samples.extend(tail);
        Ok(())
    }
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn biquad(x: &[f64], b: [f64; 3], a: [f64; 3]) -> Vec<f64> {
    let (mut z1, mut z2) = (0.0, 0.0);
    x.iter()
        .map(|&xi| {
            let y = b[0] * xi + z1;
            z1 = b[1] * xi - a[1] * y + z2;
            z2 = b[2] * xi - a[2] * y;
            y
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn preprocess_stats() {
        let path = std::env::var("IRODORI_TEST_WAV").unwrap();
        let wav = Audio::from_wav(&std::fs::read(path).unwrap()).unwrap();
        let len_in = wav.samples.len();
        let r = wav.resample(48_000);
        let lufs = r.loudness().unwrap();
        let mut n = Audio::new(r.samples.clone(), 48_000);
        n.normalize_loudness(-16.0).unwrap();
        println!(
            "len_in={len_in} len_out={} r[1000..1004]={:?} lufs={lufs:.6} n[1000..1004]={:?}",
            r.samples.len(),
            &r.samples[1000..1004],
            &n.samples[1000..1004]
        );
    }

    #[test]
    fn resample_identity_and_length() {
        let x: Vec<f32> = (0..44_100).map(|i| (i as f32 * 0.01).sin()).collect();
        assert_eq!(Audio::new(x.clone(), 48_000).resample(48_000).samples, x);
        assert_eq!(Audio::new(x, 44_100).resample(48_000).samples.len(), 48_000);
    }

    #[test]
    fn wav_round_trip() {
        let x = [0.0f32, 0.5, -0.5, 1.0, -1.0];
        let wav = Audio::new(x.to_vec(), 48_000).to_wav().unwrap();
        let y = Audio::from_wav(&wav).unwrap();
        assert_eq!(y.sample_rate, 48_000);
        assert_eq!(y.samples.len(), x.len());
        for (a, b) in x.iter().zip(&y.samples) {
            assert!((a - b).abs() < 1e-4, "{a} vs {b}");
        }
    }

    #[test]
    fn reflect_pad() {
        let mut a = Audio::new(vec![1.0, 2.0, 3.0, 4.0, 5.0], 48_000);
        a.reflect_pad_to_multiple(4).unwrap();
        assert_eq!(a.samples, [1.0, 2.0, 3.0, 4.0, 5.0, 4.0, 3.0, 2.0]);
        let mut b = Audio::new(vec![1.0, 2.0], 48_000);
        assert!(b.reflect_pad_to_multiple(8).is_err());
    }
}
