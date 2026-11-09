//! UI-facing handle to the ordered background worker.
use super::{
    protocol::{Command, Event},
    worker::Worker,
};
use crate::synthesis::Inference;
use anyhow::{Context, Result};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

pub struct Service {
    sender: mpsc::Sender<Command>,
    events: mpsc::Receiver<Event>,
    stop: Arc<AtomicBool>,
}
impl Service {
    pub fn start(inference: Arc<Inference>) -> Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let (events, output) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        std::thread::Builder::new()
            .name("irodoru-app".into())
            .spawn(move || {
                Worker::run(receiver, events, inference, worker_stop);
            })?;
        Ok(Self {
            sender,
            events: output,
            stop,
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
}
