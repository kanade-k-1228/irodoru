//! Audio device creation and decoding stay on their own thread.
use anyhow::Result;
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
        let duration = reader.duration() as f32 / reader.spec().sample_rate as f32;
        let (sender, receiver) = mpsc::channel();
        let status = Arc::new(Mutex::new(Status::default()));
        let report = status.clone();
        std::thread::Builder::new()
            .name("irodoru-audio".into())
            .spawn(move || {
                let result = (|| -> Result<()> {
                    let device = rodio::DeviceSinkBuilder::open_default_sink()?;
                    let player = rodio::Player::connect_new(device.mixer());
                    player.append(rodio::Decoder::try_from(Cursor::new(bytes.clone()))?);
                    loop {
                        match receiver.recv_timeout(Duration::from_millis(30)) {
                            Ok(Command::Toggle) => {
                                if player.empty() {
                                    player.append(rodio::Decoder::try_from(Cursor::new(
                                        bytes.clone(),
                                    ))?);
                                    player.play();
                                } else if player.is_paused() {
                                    player.play();
                                } else {
                                    player.pause();
                                }
                            }
                            Ok(Command::Seek(fraction)) => {
                                player.try_seek(Duration::from_secs_f32(
                                    (fraction * duration).clamp(0.0, duration),
                                ))?;
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
        self.sender.send(Command::Seek(fraction))?;
        Ok(())
    }
}
