//! UI-facing handle to the ordered background worker.
use super::{
    protocol::{Command, Event},
    worker::Worker,
};
use crate::{domain::InferenceStatus, synthesis::Inference};
use anyhow::{Context, Result};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

pub struct Service {
    sender: mpsc::Sender<Command>,
    events: mpsc::Receiver<Event>,
    stop: Arc<AtomicBool>,
    inference: Arc<Inference>,
}
impl Service {
    pub fn start() -> Result<Self> {
        static ENGINE: std::sync::OnceLock<Mutex<std::sync::Weak<Inference>>> =
            std::sync::OnceLock::new();
        let mut shared = ENGINE
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| anyhow::anyhow!("engine lock poisoned"))?;
        let inference = if let Some(engine) = shared.upgrade() {
            engine
        } else {
            Arc::new(Inference::start_with_progress(|progress| {
                Ok(irodoru_engine::EngineConfig::anime(
                    crate::synthesis::resources::models_with_progress(progress)?,
                ))
            })?)
        };
        *shared = Arc::downgrade(&inference);
        drop(shared);
        let (sender, receiver) = mpsc::channel();
        let (events, output) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let worker_inference = inference.clone();
        std::thread::Builder::new()
            .name("irodoru-app".into())
            .spawn(move || {
                Worker::run(receiver, events, worker_inference, worker_stop);
            })?;
        Ok(Self {
            sender,
            events: output,
            stop,
            inference,
        })
    }
    /// Drain notifications without exposing the transport to presentation code.
    pub fn drain_events(&self) -> impl Iterator<Item = Event> + '_ {
        self.events.try_iter()
    }
    pub fn send(&self, command: Command) -> Result<()> {
        if matches!(command, Command::Generate(_)) {
            self.stop.store(false, Ordering::SeqCst);
        }
        self.sender
            .send(command)
            .context("アプリ処理が終了しました")
    }
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
    pub fn retry_models(&self) -> Result<()> {
        self.inference.retry()
    }
    pub fn status(&self) -> InferenceStatus {
        self.inference.status()
    }
}
