mod error;
mod request;
mod state;
mod tts;
use axum::Router;
pub use state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

fn documented_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(tts::tts))
}
fn router() -> Router<AppState> {
    documented_router()
        .split_for_parts()
        .0
        .layer(state::body_limit())
}
pub fn openapi() -> utoipa::openapi::OpenApi {
    documented_router().split_for_parts().1
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        engine::TTSJob,
        error::AppError,
        generation::GenerationConditions,
        inference::{Audio, SynthOutput},
    };
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    struct Fake;
    impl state::Synthesis for Fake {
        fn synthesize(
            &mut self,
            job: TTSJob,
        ) -> Result<(GenerationConditions, SynthOutput), AppError> {
            Ok((
                GenerationConditions::from_params("anime".into(), &job.params, 42, 4, 48000),
                SynthOutput {
                    audio: Audio::new(vec![0.0; 48000], 48000),
                    seed: 42,
                    num_steps: 4,
                    timings: vec![],
                },
            ))
        }
    }
    async fn post(body: String) -> axum::response::Response {
        AppState::new(Fake)
            .router()
            .oneshot(
                Request::post("/")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap()
    }
    #[tokio::test]
    async fn returns_wav_with_embedded_conditions() {
        let response = post(r#"{"text":"こんにちは"}"#.into()).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "audio/wav");
        let wav = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(Audio::from_wav(&wav).unwrap().samples.len(), 48000);
        let at = wav.windows(4).position(|w| w == b"ICMT").unwrap() + 8;
        let size = u32::from_le_bytes(wav[at - 4..at].try_into().unwrap()) as usize;
        let conditions: serde_json::Value =
            serde_json::from_slice(&wav[at..at + size - 1]).unwrap();
        assert_eq!(conditions["model"], "anime");
        assert_eq!(conditions["seed"], 42);
        assert_eq!(conditions["num_steps"], 4);
    }
    #[tokio::test]
    async fn rejects_invalid_requests_with_json_errors() {
        for (body, status) in [
            (
                r#"{"text":"hello","reference_audio_base64":"!"}"#.into(),
                StatusCode::BAD_REQUEST,
            ),
            (
                r#"{"text":"hello","model":"anime"}"#.into(),
                StatusCode::UNPROCESSABLE_ENTITY,
            ),
            (
                "x".repeat(64 * 1024 * 1024 + 1),
                StatusCode::PAYLOAD_TOO_LARGE,
            ),
        ] {
            let response = post(body).await;
            assert_eq!(response.status(), status);
            let body = to_bytes(response.into_body(), 10_000).await.unwrap();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert!(json["error"].is_string());
        }
    }
}
