use crate::types::InferenceStatus;
use anyhow::{Context, Result, bail};
use irodoru_engine::{Engine, EngineConfig, GenerationConditions, SynthOutput, TTSJob};
use std::sync::{Arc, Mutex, mpsc};

type Output = Result<(GenerationConditions, SynthOutput)>;
enum Command {
    Retry,
    Synthesize(Box<TTSJob>, mpsc::SyncSender<Output>),
}
pub struct Inference {
    sender: mpsc::SyncSender<Command>,
    status: Arc<Mutex<InferenceStatus>>,
}
impl Inference {
    pub fn start_with(config: impl Fn() -> Result<EngineConfig> + Send + 'static) -> Result<Self> {
        Self::start_with_progress(move |_| config())
    }
    pub fn start_with_progress(
        config: impl Fn(&dyn Fn(InferenceStatus)) -> Result<EngineConfig> + Send + 'static,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<Command>(1);
        let status = Arc::new(Mutex::new(InferenceStatus::Loading));
        let state = status.clone();
        std::thread::Builder::new()
            .name("irodoru-engine".into())
            .spawn(move || {
                let progress = |status| *state.lock().unwrap() = status;
                let load = || {
                    let config = config(&progress)?;
                    progress(InferenceStatus::Loading);
                    Engine::load(&config)
                };
                let mut engine = None;
                let mut initialize = true;
                loop {
                    if initialize {
                        match load() {
                            Ok(loaded) => {
                                engine = Some(loaded);
                                progress(InferenceStatus::Ready);
                            }
                            Err(error) => progress(InferenceStatus::Error {
                                error: format!("{error:#}"),
                            }),
                        }
                    }
                    match receiver.recv() {
                        Ok(Command::Retry) => initialize = true,
                        Ok(Command::Synthesize(job, reply)) => {
                            initialize = false;
                            let result = engine
                                .as_mut()
                                .context("モデルを利用できません")
                                .and_then(|engine| engine.synthesize(*job).map_err(Into::into));
                            progress(InferenceStatus::Ready);
                            let _ = reply.send(result);
                        }
                        Err(_) => break,
                    }
                }
            })?;
        Ok(Self { sender, status })
    }
    pub fn status(&self) -> InferenceStatus {
        self.status.lock().unwrap().clone()
    }
    pub fn retry(&self) -> Result<()> {
        let mut status = self.status.lock().unwrap();
        if !matches!(*status, InferenceStatus::Error { .. }) {
            bail!("モデルの準備は既に進行中か、完了しています");
        }
        self.sender
            .try_send(Command::Retry)
            .context("モデルの再試行を受け付けられません")?;
        *status = InferenceStatus::Loading;
        Ok(())
    }
    pub fn synthesize(&self, job: TTSJob) -> Output {
        let mut status = self.status.lock().unwrap();
        match &*status {
            InferenceStatus::Ready => {}
            InferenceStatus::Error { error } => bail!("{error}"),
            _ => bail!("モデルを準備中、または生成中です"),
        }
        let (send, receive) = mpsc::sync_channel(1);
        self.sender
            .try_send(Command::Synthesize(Box::new(job), send))
            .map_err(|_| anyhow::anyhow!("推論処理を受け付けられません"))?;
        *status = InferenceStatus::Generating;
        drop(status);
        receive.recv().context("推論処理が終了しました")?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::{Duration, Instant},
    };

    fn wait_error(inference: &Inference) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !matches!(inference.status(), InferenceStatus::Error { .. }) {
            assert!(Instant::now() < deadline, "worker did not report failure");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn initialization_failure_keeps_worker_alive_for_retry() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let count = attempts.clone();
        let inference = Inference::start_with_progress(move |progress| {
            count.fetch_add(1, Ordering::SeqCst);
            progress(InferenceStatus::Downloading {
                downloaded: 2,
                total: 5,
            });
            bail!("connection failed")
        })
        .unwrap();
        wait_error(&inference);
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        inference.retry().unwrap();
        wait_error(&inference);
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }
}
