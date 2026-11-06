use std::path::PathBuf;
use tauri::Manager;

pub fn models<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> anyhow::Result<PathBuf> {
    #[cfg(debug_assertions)]
    {
        let local = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/models");
        if local.join("anime/manifest.json").is_file() {
            return Ok(local);
        }
    }
    let bundled = app.path().resource_dir()?.join("models");
    #[cfg(not(target_os = "android"))]
    {
        Ok(bundled)
    }
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_fs::FsExt;
        let index = app.fs().read(bundled.join("bundle.json"))?;
        crate::model_resources::materialize(
            &index,
            &app.path().app_local_data_dir()?.join("bundled-models"),
            |path| {
                let mut options = tauri_plugin_fs::OpenOptions::new();
                options.read(true);
                Ok(app.fs().open(bundled.join(path), options)?)
            },
        )
    }
}
