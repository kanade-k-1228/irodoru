use crate::domain::InferenceStatus;
use anyhow::{Context, Result, bail};
use irodoru_engine::{Engine, EngineConfig, GenerationConditions, SynthOutput, TTSJob};
use std::sync::{Arc, Mutex, mpsc};

type Output = Result<(GenerationConditions, SynthOutput)>;
type Command = (TTSJob, mpsc::SyncSender<Output>);
pub struct Inference {
    sender: mpsc::SyncSender<Command>,
    status: Arc<Mutex<InferenceStatus>>,
}
impl Inference {
    pub fn start_with(
        config: impl FnOnce() -> Result<EngineConfig> + Send + 'static,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<Command>(1);
        let status = Arc::new(Mutex::new(InferenceStatus::Loading));
        let state = status.clone();
        std::thread::Builder::new()
            .name("irodoru-engine".into())
            .spawn(move || {
                let mut engine = match config().and_then(|config| Engine::load(&config)) {
                    Ok(engine) => engine,
                    Err(error) => {
                        *state.lock().unwrap() = InferenceStatus::Error {
                            error: format!("{error:#}"),
                        };
                        return;
                    }
                };
                *state.lock().unwrap() = InferenceStatus::Ready;
                while let Ok((job, reply)) = receiver.recv() {
                    let result = engine.synthesize(job).map_err(Into::into);
                    *state.lock().unwrap() = InferenceStatus::Ready;
                    let _ = reply.send(result);
                }
            })?;
        Ok(Self { sender, status })
    }
    pub fn status(&self) -> InferenceStatus {
        self.status.lock().unwrap().clone()
    }
    pub fn synthesize(&self, job: TTSJob) -> Output {
        let mut status = self.status.lock().unwrap();
        match &*status {
            InferenceStatus::Ready => {}
            InferenceStatus::Error { error } => bail!("{error}"),
            _ => bail!("モデルを読み込み中、または生成中です"),
        }
        let (send, receive) = mpsc::sync_channel(1);
        self.sender
            .try_send((job, send))
            .map_err(|_| anyhow::anyhow!("推論処理を受け付けられません"))?;
        *status = InferenceStatus::Generating;
        drop(status);
        receive.recv().context("推論処理が終了しました")?
    }
}
