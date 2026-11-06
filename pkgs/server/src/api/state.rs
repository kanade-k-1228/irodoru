use super::error::ApiError;
use crate::generation::GenerationConditions;
use crate::{
    engine::{Engine, TTSJob},
    error::AppError,
    inference::SynthOutput,
};
use axum::{Router, extract::DefaultBodyLimit};
use std::sync::{Arc, Mutex};
pub(crate) trait Synthesis: Send {
    fn synthesize(&mut self, job: TTSJob) -> Result<(GenerationConditions, SynthOutput), AppError>;
}
impl Synthesis for Engine {
    fn synthesize(&mut self, job: TTSJob) -> Result<(GenerationConditions, SynthOutput), AppError> {
        Engine::synthesize(self, job)
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
        let engine = self.engine.clone();
        tokio::task::spawn_blocking(move || {
            engine.lock().map_err(AppError::internal)?.synthesize(job)
        })
        .await
        .map_err(ApiError::internal)?
        .map_err(Into::into)
    }
}
pub(super) fn body_limit() -> DefaultBodyLimit {
    DefaultBodyLimit::max(64 * 1024 * 1024)
}
