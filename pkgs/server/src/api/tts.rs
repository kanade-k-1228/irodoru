use std::time::Instant;

use axum::Json;
use axum::extract::{State, rejection::JsonRejection};
use axum::response::{IntoResponse, Response};
use tracing::{error, info};
use utoipa::ToSchema;

use super::AppState;
use super::error::{ApiError, ErrorBody};
use super::request::TTSRequest;
use irodoru_engine::TTSJob;

/// Generate one WAV (48 kHz, mono, 16-bit PCM).
///
/// The resolved generation conditions (model, seed, steps and the other parameters)
/// are embedded as JSON in the WAV's LIST/INFO ICMT chunk.
#[utoipa::path(post, path = "/", request_body = TTSRequest,
    responses((status = 200, body = Wav, content_type = "audio/wav", headers(
        ("content-disposition" = String, description = "Filename is SHA-256 of the embedded generation conditions"))),
        (status = 400, body = ErrorBody), (status = 413, description = "Request body too large", body = ErrorBody),
        (status = 415, description = "Expected application/json", body = ErrorBody),
        (status = 422, description = "Invalid JSON fields", body = ErrorBody),
        (status = 500, body = ErrorBody)))]
pub async fn tts(
    State(state): State<AppState>,
    req: Result<Json<TTSRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(req) = req.map_err(ApiError::from_json)?;
    let job = TTSJob::try_from(req)?;
    let started = Instant::now();
    let (conditions, out) = state
        .synthesize(job)
        .await
        .inspect_err(|e| error!("tts failed: {}", e.message()))?;

    let elapsed_ms = started.elapsed().as_millis() as u64;
    let audio_secs = out.audio.seconds();
    info!(
        model = %conditions.model, seed = out.seed, steps = out.num_steps, audio_secs, elapsed_ms,
        rtf = elapsed_ms as f64 / 1000.0 / audio_secs.max(1e-6), timings = ?out.timings, "synthesized"
    );
    let wav = conditions.to_wav(&out.audio).map_err(ApiError::internal)?;
    let filename = conditions.filename().map_err(ApiError::internal)?;
    let headers = [
        (
            "content-disposition",
            format!("attachment; filename=\"{filename}\""),
        ),
        ("content-type", "audio/wav".to_string()),
    ];
    Ok((headers, wav).into_response())
}

#[allow(dead_code)]
#[derive(ToSchema)]
#[schema(value_type = String, format = Binary)]
pub(super) struct Wav(Vec<u8>);
