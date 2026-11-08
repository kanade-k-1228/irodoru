use crate::types::InferenceStatus;
use std::path::PathBuf;
use tauri::Manager;

pub fn models<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    progress: &dyn Fn(InferenceStatus),
) -> anyhow::Result<PathBuf> {
    #[cfg(debug_assertions)]
    {
        let local = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/models");
        if local.join("anime/manifest.json").is_file() {
            return Ok(local);
        }
    }
    crate::model_resources::download(
        include_bytes!("../resources/model-distribution.json"),
        &app.path().app_local_data_dir()?.join("models"),
        progress,
    )
}
