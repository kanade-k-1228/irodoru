use super::error::ApiError;
use axum::{Router, extract::DefaultBodyLimit};
use irodoru_engine::{AppError, Engine, GenerationConditions, SynthOutput, TTSJob};
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot};
pub(crate) trait Synthesis: Send {
    fn synthesize(
        &mut self,
        job: TTSJob,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<(GenerationConditions, SynthOutput), AppError>;
}
impl Synthesis for Engine {
    fn synthesize(
        &mut self,
        job: TTSJob,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<(GenerationConditions, SynthOutput), AppError> {
        Engine::synthesize_cancellable(self, job, is_cancelled)
    }
}
#[derive(Clone)]
pub struct AppState {
    engine: Arc<Mutex<Box<dyn Synthesis>>>,
}
impl AppState {
    pub(crate) fn new(engine: impl Synthesis + 'static) -> Self {
        Self {
            engine: Arc::new(Mutex::new(Box::new(engine))),
        }
    }
    pub fn router(self) -> Router {
        super::router().with_state(self)
    }
    pub(super) async fn synthesize(
        &self,
        job: TTSJob,
    ) -> Result<(GenerationConditions, SynthOutput), ApiError> {
        // Disconnecting cancels the wait.
        let mut engine = self.engine.clone().lock_owned().await;
        let (sender, receiver) = oneshot::channel();
        tokio::task::spawn_blocking(move || {
            // Disconnecting drops the receiver. Hold the lock until synthesis stops.
            if !sender.is_closed() {
                let result = engine.synthesize(job, &|| sender.is_closed());
                let _ = sender.send(result);
            }
        });
        receiver
            .await
            .map_err(|_| ApiError::internal("synthesis worker stopped without a result"))?
            .map_err(Into::into)
    }
}
pub(super) fn body_limit() -> DefaultBodyLimit {
    DefaultBodyLimit::max(64 * 1024 * 1024)
}
