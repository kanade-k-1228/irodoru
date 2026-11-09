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

#[cfg(test)]
mod tests {
    use super::*;
    use irodoru_engine::Audio;
    use std::time::Duration;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        sync::mpsc,
        time::timeout,
    };

    struct Fake {
        started: mpsc::UnboundedSender<String>,
        stopped: mpsc::UnboundedSender<()>,
    }

    impl Synthesis for Fake {
        fn synthesize(
            &mut self,
            job: TTSJob,
            is_cancelled: &dyn Fn() -> bool,
        ) -> Result<(GenerationConditions, SynthOutput), AppError> {
            self.started.send(job.params.text.clone()).unwrap();
            if job.params.text == "running" {
                // Limit the wait so failed cancellation cannot hang the test.
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                while !is_cancelled() {
                    assert!(std::time::Instant::now() < deadline, "not cancelled");
                    std::thread::sleep(Duration::from_millis(1));
                }
                self.stopped.send(()).unwrap();
                return Err(AppError::internal("cancelled"));
            }
            Ok((
                GenerationConditions::from_params("fake".into(), &job.params, 1, 1, 48000),
                SynthOutput {
                    audio: Audio::new(vec![0.0; 48], 48000),
                    seed: 1,
                    num_steps: 1,
                    timings: vec![],
                },
            ))
        }
    }

    async fn connect(addr: std::net::SocketAddr, text: &str) -> TcpStream {
        let mut socket = TcpStream::connect(addr).await.unwrap();
        let body = format!(r#"{{"text":"{text}"}}"#);
        socket.write_all(format!(
            "POST / HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(), body
        ).as_bytes()).await.unwrap();
        socket
    }

    #[tokio::test]
    async fn http_disconnect_cancels_running_and_discards_waiting_requests() {
        let (started_tx, mut started) = mpsc::unbounded_channel();
        let (stopped_tx, mut stopped) = mpsc::unbounded_channel();
        let state = AppState::new(Fake {
            started: started_tx,
            stopped: stopped_tx,
        });
        let engine = state.engine.clone();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, state.router()).await.unwrap();
        });

        timeout(Duration::from_secs(3), async {
            let running = connect(addr, "running").await;
            assert_eq!(started.recv().await.as_deref(), Some("running"));

            // Wait for the second request to queue before disconnecting.
            let baseline = Arc::strong_count(&engine);
            let waiting = connect(addr, "waiting").await;
            while Arc::strong_count(&engine) <= baseline {
                tokio::task::yield_now().await;
            }
            drop(waiting);
            while Arc::strong_count(&engine) > baseline {
                tokio::task::yield_now().await;
            }
            assert!(started.try_recv().is_err());

            drop(running);
            stopped.recv().await.unwrap();

            // Verify the engine still handles requests.
            let mut next = connect(addr, "next").await;
            let mut response = Vec::new();
            next.read_to_end(&mut response).await.unwrap();
            assert!(response.starts_with(b"HTTP/1.1 200"));
            assert!(response.windows(4).any(|w| w == b"RIFF"));
            assert_eq!(started.recv().await.as_deref(), Some("next"));
            assert!(started.try_recv().is_err());
        })
        .await
        .expect("HTTP disconnect did not cancel synthesis");
        server.abort();
    }
}
