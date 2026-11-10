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
