use crate::{domain::InferenceStatus, preferences::ModelSettings};
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
    settings: ModelSettings,
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
        Self::start_configured(
            ModelSettings::default(),
            move |_, progress| config(progress),
            |_| Ok(()),
        )
    }

    pub fn start_configured(
        settings: ModelSettings,
        config: impl Fn(Option<&Path>, &dyn Fn(InferenceStatus)) -> Result<EngineConfig>
        + Send
        + 'static,
        save_settings: impl Fn(&ModelSettings) -> Result<()> + Send + 'static,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<Command>(1);
        let state = Arc::new(Mutex::new(ModelState {
            status: InferenceStatus::Loading,
            settings,
        }));
        let worker_state = state.clone();
        std::thread::Builder::new()
            .name("irodoru-engine".into())
            .spawn(move || {
                let progress = |status| worker_state.lock().unwrap().status = status;
                let load = || {
                    let directory = worker_state.lock().unwrap().settings.directory.clone();
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
                        Ok(Command::Configure(archive)) => {
                            initialize = false;
                            let mut configure = || -> Result<ModelSettings> {
                                let imported = archive
                                    .as_deref()
                                    .map(|path| super::model_import::import(path, &progress))
                                    .transpose()?;
                                let settings = ModelSettings {
                                    directory: imported.as_ref().map(|dir| dir.path().to_owned()),
                                    archive_name: archive
                                        .as_ref()
                                        .and_then(|path| path.file_name())
                                        .map(|name| name.to_string_lossy().into_owned()),
                                };
                                switch_engine(
                                    &mut engine,
                                    || {
                                        let config =
                                            config(settings.directory.as_deref(), &progress)?;
                                        progress(InferenceStatus::Loading);
                                        Engine::load(&config)
                                    },
                                    || save_settings(&settings),
                                )?;
                                if let Some(imported) = imported {
                                    let _ = imported.keep();
                                }
                                Ok(settings)
                            };
                            match configure() {
                                Ok(settings) => {
                                    let mut state = worker_state.lock().unwrap();
                                    state.settings = settings;
                                    state.status = InferenceStatus::Ready;
                                }
                                Err(error) => progress(InferenceStatus::Error {
                                    error: format!("{error:#}"),
                                }),
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
    pub fn model_name(&self) -> Option<String> {
        let state = self.state.lock().unwrap();
        state.settings.directory.as_ref().map(|_| {
            state
                .settings
                .archive_name
                .clone()
                .unwrap_or_else(|| "ローカルモデル（旧設定）".into())
        })
    }

    pub fn configure_models(&self, archive: Option<PathBuf>) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        if !matches!(
            state.status,
            InferenceStatus::Ready | InferenceStatus::Error { .. }
        ) {
            bail!("モデルの準備・音声生成が完了してから変更してください");
        }
        self.sender
            .try_send(Command::Configure(archive))
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

// Publish neither the replacement engine nor its settings if validation/loading fails.
fn switch_engine<T>(
    engine: &mut Option<T>,
    load: impl FnOnce() -> Result<T>,
    save: impl FnOnce() -> Result<()>,
) -> Result<()> {
    drop(engine.take());
    let replacement = load()?;
    save()?;
    *engine = Some(replacement);
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
    #[test]
    fn invalid_import_keeps_saved_source_and_retry_uses_it() {
        let previous = ModelSettings {
            directory: Some(PathBuf::from("previous-models")),
            archive_name: Some("previous.zip".into()),
        };
        let loaded = Arc::new(Mutex::new(Vec::new()));
        let observed = loaded.clone();
        let inference = Inference::start_configured(
            previous.clone(),
            move |directory, _| {
                observed.lock().unwrap().push(directory.map(Path::to_owned));
                bail!("test: stop before loading real models")
            },
            |_| panic!("an invalid import must not persist settings"),
        )
        .unwrap();
        wait_error(&inference);
        let source = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(source.path(), b"not a zip").unwrap();
        inference
            .configure_models(Some(source.path().to_owned()))
            .unwrap();
        wait_error(&inference);
        assert_eq!(inference.state.lock().unwrap().settings, previous);
        assert_eq!(inference.model_name().as_deref(), Some("previous.zip"));
        assert_eq!(loaded.lock().unwrap().len(), 1);
        inference.retry().unwrap();
        wait_error(&inference);
        assert_eq!(
            *loaded.lock().unwrap(),
            vec![previous.directory.clone(), previous.directory]
        );
    }

    #[test]
    fn failed_default_load_does_not_reset_saved_selection() {
        let previous = ModelSettings {
            directory: Some(PathBuf::from("previous-models")),
            archive_name: Some("previous.zip".into()),
        };
        let inference = Inference::start_configured(
            previous.clone(),
            |_, _| bail!("test: default model unavailable"),
            |_| panic!("failed loading must not persist settings"),
        )
        .unwrap();
        wait_error(&inference);
        inference.configure_models(None).unwrap();
        wait_error(&inference);
        assert_eq!(inference.state.lock().unwrap().settings, previous);
    }

    #[test]
    fn selection_is_saved_only_after_loading_and_installed_only_after_saving() {
        let mut engine = Some(1);
        let saved = std::cell::Cell::new(false);
        assert!(
            switch_engine(
                &mut engine,
                || bail!("invalid model"),
                || {
                    saved.set(true);
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(!saved.get());
        assert!(engine.is_none());
        assert!(switch_engine(&mut engine, || Ok(2), || bail!("read-only settings")).is_err());
        assert!(engine.is_none());
        switch_engine(
            &mut engine,
            || Ok(3),
            || {
                saved.set(true);
                Ok(())
            },
        )
        .unwrap();
        assert!(saved.get());
        assert_eq!(engine, Some(3));
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
