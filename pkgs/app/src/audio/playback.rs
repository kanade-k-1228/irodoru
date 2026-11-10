//! Audio device creation and decoding stay on their own thread.
use anyhow::{Result, ensure};
use std::{
    io::Cursor,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
enum Command {
    Toggle,
    Seek(f32),
}
#[derive(Default)]
struct Status {
    position: f32,
    playing: bool,
    error: Option<String>,
}
pub struct Playback {
    sender: mpsc::Sender<Command>,
    status: Arc<Mutex<Status>>,
    pub duration: f32,
}
impl Playback {
    pub fn new(bytes: Vec<u8>) -> Result<Self> {
        let reader = hound::WavReader::new(Cursor::new(&bytes))?;
        ensure!(
            reader.spec().sample_rate > 0 && reader.duration() > 0,
            "音声データが空です"
        );
        let duration = reader.duration() as f32 / reader.spec().sample_rate as f32;
        let bytes: Arc<[u8]> = bytes.into();
        let (sender, receiver) = mpsc::channel();
        let status = Arc::new(Mutex::new(Status::default()));
        let report = status.clone();
        std::thread::Builder::new()
            .name("irodoru-audio".into())
            .spawn(move || {
                let result = (|| -> Result<()> {
                    let device = rodio::DeviceSinkBuilder::open_default_sink()?;
                    let player = rodio::Player::connect_new(device.mixer());
                    player.append(decoder(bytes.clone())?);
                    loop {
                        match receiver.recv_timeout(Duration::from_millis(30)) {
                            Ok(Command::Toggle) => {
                                if player.empty() {
                                    player.append(decoder(bytes.clone())?);
                                    player.play();
                                } else if player.is_paused() {
                                    player.play();
                                } else {
                                    player.pause();
                                }
                            }
                            Ok(Command::Seek(fraction)) => {
                                seek_player(
                                    &player,
                                    &bytes,
                                    Duration::from_secs_f32(fraction * duration),
                                )?;
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            Err(mpsc::RecvTimeoutError::Timeout) => {}
                        }
                        let mut s = report.lock().unwrap();
                        s.position = if player.empty() {
                            duration
                        } else {
                            player.get_pos().as_secs_f32().min(duration)
                        };
                        s.playing = !player.is_paused() && !player.empty();
                    }
                    Ok(())
                })();
                if let Err(e) = result {
                    let mut s = report.lock().unwrap();
                    s.playing = false;
                    s.error = Some(format!("音声を再生できません: {e:#}"));
                }
            })?;
        Ok(Self {
            sender,
            status,
            duration,
        })
    }
    pub fn playing(&self) -> bool {
        self.status.lock().unwrap().playing
    }
    pub fn position(&self) -> f32 {
        self.status.lock().unwrap().position
    }
    pub fn take_error(&self) -> Option<String> {
        self.status.lock().unwrap().error.take()
    }
    pub fn toggle(&self) {
        let _ = self.sender.send(Command::Toggle);
    }
    pub fn seek(&self, fraction: f32) -> Result<()> {
        ensure!(fraction.is_finite(), "再生位置が不正です");
        self.sender.send(Command::Seek(fraction.clamp(0.0, 1.0)))?;
        Ok(())
    }
}

fn seek_player(player: &rodio::Player, bytes: &Arc<[u8]>, position: Duration) -> Result<()> {
    // Finished sources leave the queue; restore one before seeking.
    // Scrubbing after completion stays paused until playback is requested.
    if player.empty() {
        player.pause();
        player.append(decoder(bytes.clone())?);
    }
    player.try_seek(position)?;
    Ok(())
}

fn decoder(bytes: Arc<[u8]>) -> Result<rodio::Decoder<Cursor<Arc<[u8]>>>> {
    // Cursor conversion alone leaves Symphonia's source marked as forward-only.
    let byte_len = bytes.len() as u64;
    Ok(rodio::Decoder::builder()
        .with_data(Cursor::new(bytes))
        .with_byte_len(byte_len)
        .build()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::Source;

    fn wav(channels: u16, frames: usize) -> Arc<[u8]> {
        let mut bytes = Cursor::new(Vec::new());
        let mut writer = hound::WavWriter::new(
            &mut bytes,
            hound::WavSpec {
                channels,
                sample_rate: 48_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for i in 0..frames * channels as usize {
            writer.write_sample((i % 1000) as i16).unwrap();
        }
        writer.finalize().unwrap();
        bytes.into_inner().into()
    }

    #[test]
    fn decoder_seeks_backwards_and_after_end() {
        for channels in [1, 2] {
            let bytes = wav(channels, 48_000);
            let mut source = decoder(bytes.clone()).unwrap();
            for millis in [900, 100, 0, 999, 500, 1000, 0] {
                source.try_seek(Duration::from_millis(millis)).unwrap();
                if millis < 1000 {
                    assert!(source.next().is_some(), "seek to {millis}ms");
                }
            }
            // Seeking also works after the decoder has been fully drained.
            source.by_ref().for_each(drop);
            source.try_seek(Duration::ZERO).unwrap();
            assert_eq!(source.next(), decoder(bytes).unwrap().next());
        }
    }
    #[test]
    fn decoder_seeks_short_recordings_with_metadata() {
        for frames in [1, 47_999] {
            let bytes = crate::audio::with_voice(
                &wav(2, frames),
                &crate::domain::Voice::Imported {
                    id: crate::domain::new_id(),
                    name: "参照音声".into(),
                },
            )
            .unwrap();
            let mut source = decoder(bytes.into()).unwrap();
            let duration = frames as f64 / 48_000.;
            for fraction in [1.0, 0.0, 0.999, 0.25, 0.0] {
                source
                    .try_seek(Duration::from_secs_f64(duration * fraction))
                    .unwrap();
            }
            assert!(source.next().is_some());
        }
    }

    #[test]
    fn player_can_seek_while_paused_and_after_completion() {
        let (player, mut output) = rodio::Player::new();
        let bytes = wav(1, 48_000);
        player.pause();
        player.append(decoder(bytes.clone()).unwrap());
        // Consume the player queue without needing an OS audio device.
        let (stop, stopped) = mpsc::channel::<()>();
        let consumer = std::thread::spawn(move || {
            while matches!(stopped.try_recv(), Err(mpsc::TryRecvError::Empty)) {
                for _ in 0..240 {
                    output.next();
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        for millis in [750, 250, 0, 999, 500] {
            seek_player(&player, &bytes, Duration::from_millis(millis)).unwrap();
            assert!(player.is_paused());
            assert!(
                player.get_pos().abs_diff(Duration::from_millis(millis))
                    < Duration::from_millis(10)
            );
        }
        player.play();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !player.empty() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        seek_player(&player, &bytes, Duration::from_millis(250)).unwrap();
        assert!(!player.empty());
        assert!(player.is_paused());
        assert!(player.get_pos().abs_diff(Duration::from_millis(250)) < Duration::from_millis(10));
        stop.send(()).unwrap();
        consumer.join().unwrap();
    }
}
