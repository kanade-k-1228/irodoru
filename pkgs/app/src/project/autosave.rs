use super::{
    ProjectStatus, SharedProject,
    archive::{snapshot, write_archive},
};
use anyhow::{Context, Result};
use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

pub(super) type Report = Arc<dyn Fn(ProjectStatus) + Send + Sync>;

enum Message {
    Changed,
    Flush(mpsc::Sender<Result<(), String>>),
    Stop,
}

pub struct Saver {
    sender: mpsc::Sender<Message>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Saver {
    pub fn start(
        project: SharedProject,
        report: impl Fn(ProjectStatus) + Send + Sync + 'static,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let report: Report = Arc::new(report);
        let thread = std::thread::Builder::new()
            .name("project-save".into())
            .spawn(move || {
                while let Ok(message) = receiver.recv() {
                    let mut waiters = Vec::new();
                    match message {
                        Message::Stop => break,
                        Message::Flush(reply) => waiters.push(reply),
                        Message::Changed => {
                            // Debounce action bursts, but keep typing from postponing saving forever.
                            let deadline = Instant::now() + Duration::from_secs(1);
                            loop {
                                let delay = Duration::from_millis(200)
                                    .min(deadline.saturating_duration_since(Instant::now()));
                                match receiver.recv_timeout(delay) {
                                    Ok(Message::Changed) if Instant::now() < deadline => {}
                                    Ok(Message::Changed) => break,
                                    Ok(Message::Flush(reply)) => {
                                        waiters.push(reply);
                                        break;
                                    }
                                    Ok(Message::Stop) => return,
                                    Err(_) => break,
                                }
                            }
                        }
                    }
                    let result = save_current(&project, &report).map_err(|e| format!("{e:#}"));
                    for reply in waiters {
                        let _ = reply.send(result.clone());
                    }
                }
            })?;
        Ok(Self {
            sender,
            thread: Some(thread),
        })
    }
    pub fn changed(&self) -> Result<()> {
        self.sender
            .send(Message::Changed)
            .context("保存ワーカーが終了しています")
    }
    pub fn flush_sender(&self) -> Result<mpsc::Receiver<Result<(), String>>> {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(Message::Flush(sender))
            .context("保存ワーカーが終了しています")?;
        Ok(receiver)
    }
    pub fn flush(&self) -> Result<()> {
        self.flush_sender()?
            .recv()
            .context("保存ワーカーが終了しています")?
            .map_err(anyhow::Error::msg)
    }
}
impl Drop for Saver {
    fn drop(&mut self) {
        let _ = self.sender.send(Message::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(super) fn save_current(project: &SharedProject, report: &Report) -> Result<()> {
    let snapshot = {
        let guard = project
            .lock()
            .map_err(|_| anyhow::anyhow!("project lock poisoned"))?;
        let Some(session) = guard.as_ref() else {
            return Ok(());
        };
        if session.revision == session.saved {
            return Ok(());
        }
        report(session.status());
        snapshot(session)
    };
    let result = snapshot
        .as_ref()
        .map_err(|e| anyhow::anyhow!("{e:#}"))
        .and_then(write_archive);
    let mut guard = project
        .lock()
        .map_err(|_| anyhow::anyhow!("project lock poisoned"))?;
    if let Some(session) = guard.as_mut() {
        if let Ok(snapshot) = &snapshot {
            if session.id != snapshot.id {
                return result;
            }
            if result.is_ok() {
                session.saved = snapshot.revision;
            }
        }
        session.error = result.as_ref().err().map(|e| format!("{e:#}"));
        report(session.status());
    }
    result
}
