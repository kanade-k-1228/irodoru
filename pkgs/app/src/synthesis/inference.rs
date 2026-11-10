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
