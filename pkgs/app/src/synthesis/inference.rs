use crate::domain::InferenceStatus;
use anyhow::{Context, Result, bail};
use irodoru_engine::{Engine, EngineConfig, GenerationConditions, SynthOutput, TTSJob};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};

type Output = Result<(GenerationConditions, SynthOutput)>;
enum Command {
    Retry,
    Configure(Option<PathBuf>),
    Synthesize(Box<TTSJob>, mpsc::SyncSender<Output>),
}
struct ModelState {
    status: InferenceStatus,
    directory: Option<PathBuf>,
}

pub struct Inference {
    sender: mpsc::SyncSender<Command>,
    state: Arc<Mutex<ModelState>>,
}
impl Inference {
    #[cfg(test)]
    pub(crate) fn start_with(
        config: impl Fn() -> Result<EngineConfig> + Send + 'static,
    ) -> Result<Self> {
        Self::start_with_progress(move |_| config())
    }
    #[cfg(test)]
    pub(crate) fn start_with_progress(
        config: impl Fn(&dyn Fn(InferenceStatus)) -> Result<EngineConfig> + Send + 'static,
    ) -> Result<Self> {
        Self::start_configured(None, move |_, progress| config(progress), Ok)
    }

    pub fn start_configured(
        directory: Option<PathBuf>,
        config: impl Fn(Option<&Path>, &dyn Fn(InferenceStatus)) -> Result<EngineConfig>
        + Send
        + 'static,
        configure_directory: impl Fn(Option<PathBuf>) -> Result<Option<PathBuf>> + Send + 'static,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<Command>(1);
        let state = Arc::new(Mutex::new(ModelState {
            status: InferenceStatus::Loading,
            directory,
        }));
        let worker_state = state.clone();
        std::thread::Builder::new()
            .name("irodoru-engine".into())
            .spawn(move || {
                let progress = |status| worker_state.lock().unwrap().status = status;
                let load = || {
                    let directory = worker_state.lock().unwrap().directory.clone();
                    let config = config(directory.as_deref(), &progress)?;
                    progress(InferenceStatus::Loading);
                    Engine::load(&config)
                };
                let mut engine = None;
                let mut initialize = true;
                loop {
                    if initialize {
                        match restart_engine(&mut engine, load) {
                            Ok(()) => progress(InferenceStatus::Ready),
                            Err(error) => progress(InferenceStatus::Error {
                                error: format!("{error:#}"),
                            }),
                        }
                    }
                    match receiver.recv() {
                        Ok(Command::Retry) => initialize = true,
                        Ok(Command::Configure(directory)) => {
                            let configure = || -> Result<()> {
                                let directory = configure_directory(directory)?;
                                worker_state.lock().unwrap().directory = directory;
                                Ok(())
                            };
                            match configure() {
                                Ok(()) => initialize = true,
                                Err(error) => {
                                    initialize = false;
                                    progress(InferenceStatus::Error {
                                        error: format!("{error:#}"),
                                    });
                                }
                            }
                        }
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
        Ok(Self { sender, state })
    }
    pub fn status(&self) -> InferenceStatus {
        self.state.lock().unwrap().status.clone()
    }
    pub fn model_directory(&self) -> Option<PathBuf> {
        self.state.lock().unwrap().directory.clone()
    }

    pub fn configure_models(&self, directory: Option<PathBuf>) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        if !matches!(
            state.status,
            InferenceStatus::Ready | InferenceStatus::Error { .. }
        ) {
            bail!("モデルの準備・音声生成が完了してから変更してください");
        }
        self.sender
            .try_send(Command::Configure(directory))
            .context("モデル設定の変更を受け付けられません")?;
        state.status = InferenceStatus::Loading;
        Ok(())
    }

    pub fn retry(&self) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        if !matches!(state.status, InferenceStatus::Error { .. }) {
            bail!("モデルの準備は既に進行中か、完了しています");
        }
        self.sender
            .try_send(Command::Retry)
            .context("モデルの再試行を受け付けられません")?;
        state.status = InferenceStatus::Loading;
        Ok(())
    }
    pub fn synthesize(&self, job: TTSJob) -> Output {
        let mut state = self.state.lock().unwrap();
        match &state.status {
            InferenceStatus::Ready => {}
            InferenceStatus::Error { error } => bail!("{error}"),
            _ => bail!("モデルを準備中、または生成中です"),
        }
        let (send, receive) = mpsc::sync_channel(1);
        self.sender
            .try_send(Command::Synthesize(Box::new(job), send))
            .map_err(|_| anyhow::anyhow!("推論処理を受け付けられません"))?;
        state.status = InferenceStatus::Generating;
        drop(state);
        receive.recv().context("推論処理が終了しました")?
    }
}

/// Drop the whole instance before constructing another one, including on retry.
/// The engine never escapes its owning worker, so it needs no shared lock.
fn restart_engine<T>(engine: &mut Option<T>, load: impl FnOnce() -> Result<T>) -> Result<()> {
    drop(engine.take());
    *engine = Some(load()?);
    Ok(())
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
    fn model_folder() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("anime")).unwrap();
        std::fs::create_dir_all(dir.path().join("codec")).unwrap();
        std::fs::write(dir.path().join("anime/manifest.json"), "{}").unwrap();
        std::fs::write(dir.path().join("codec/codec.json"), "{}").unwrap();
        dir
    }

    #[test]
    fn model_source_switch_reset_and_invalid_folder() {
        let loaded = Arc::new(Mutex::new(Vec::new()));
        let observed = loaded.clone();
        let saved = Arc::new(Mutex::new(Vec::new()));
        let persisted = saved.clone();
        let inference = Inference::start_configured(
            None,
            move |directory, _| {
                observed.lock().unwrap().push(directory.map(Path::to_owned));
                bail!("test: stop before loading real models")
            },
            move |directory| {
                let directory = directory
                    .as_deref()
                    .map(super::super::resources::local_models)
                    .transpose()?;
                persisted.lock().unwrap().push(directory.clone());
                Ok(directory)
            },
        )
        .unwrap();
        wait_error(&inference);
        let folder = model_folder();
        let canonical = folder.path().canonicalize().unwrap();
        inference
            .configure_models(Some(folder.path().to_owned()))
            .unwrap();
        wait_error(&inference);
        assert_eq!(inference.model_directory(), Some(canonical.clone()));
        assert_eq!(*saved.lock().unwrap(), vec![Some(canonical.clone())]);
        assert_eq!(
            loaded.lock().unwrap().last(),
            Some(&Some(canonical.clone()))
        );

        let invalid = tempfile::tempdir().unwrap();
        inference
            .configure_models(Some(invalid.path().to_owned()))
            .unwrap();
        wait_error(&inference);
        assert_eq!(inference.model_directory(), Some(canonical));
        assert_eq!(saved.lock().unwrap().len(), 1);
        assert_eq!(loaded.lock().unwrap().len(), 2);

        inference.configure_models(None).unwrap();
        wait_error(&inference);
        assert!(inference.model_directory().is_none());
        assert_eq!(saved.lock().unwrap().last(), Some(&None));
        assert_eq!(loaded.lock().unwrap().last(), Some(&None));
    }

    #[test]
    fn failed_settings_save_keeps_previous_model_source() {
        let inference = Inference::start_configured(
            None,
            |_, _| bail!("test: no models"),
            |_| bail!("settings are read-only"),
        )
        .unwrap();
        wait_error(&inference);
        let folder = model_folder();
        inference
            .configure_models(Some(folder.path().to_owned()))
            .unwrap();
        wait_error(&inference);
        assert!(inference.model_directory().is_none());
        assert!(
            matches!(inference.status(), InferenceStatus::Error { error } if error.contains("read-only"))
        );
    }

    #[test]
    fn model_source_cannot_change_during_preparation_or_generation() {
        let (started, running) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        let inference = Inference::start_with_progress(move |_| {
            started.send(()).unwrap();
            blocked.recv().unwrap();
            bail!("test: no models")
        })
        .unwrap();
        running.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(inference.configure_models(None).is_err());
        inference.state.lock().unwrap().status = InferenceStatus::Generating;
        assert!(inference.configure_models(None).is_err());
        release.send(()).unwrap();
        wait_error(&inference);
    }
    #[test]
    fn restart_drops_the_entire_previous_instance_before_loading() {
        struct Instance(Arc<AtomicUsize>);
        impl Drop for Instance {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let dropped = Arc::new(AtomicUsize::new(0));
        let mut engine = Some(Instance(dropped.clone()));
        restart_engine(&mut engine, || {
            assert_eq!(dropped.load(Ordering::SeqCst), 1);
            Ok(Instance(dropped.clone()))
        })
        .unwrap();
        assert!(engine.is_some());
        let error = restart_engine(&mut engine, || {
            assert_eq!(dropped.load(Ordering::SeqCst), 2);
            bail!("replacement failed")
        });
        assert!(error.is_err());
        assert!(engine.is_none());
    }
}
