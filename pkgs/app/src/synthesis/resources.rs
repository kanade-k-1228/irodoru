use crate::domain::InferenceStatus;
use anyhow::{Context, Result, ensure};
use std::path::{Path, PathBuf};

pub(crate) fn local_models(path: &Path) -> Result<PathBuf> {
    ensure!(
        path.join("anime/manifest.json").is_file() && path.join("codec/codec.json").is_file(),
        "モデルが見つかりません: {}。anime/manifest.json と codec/codec.json を含むフォルダを選択してください",
        path.display()
    );
    Ok(path.canonicalize()?)
}

/// Development overrides are explicit; release builds use a verified, revisioned cache.
pub fn models_with_progress(progress: &dyn Fn(InferenceStatus)) -> Result<PathBuf> {
    let settings = crate::preferences::ModelSettings::load()?;
    models_with_directory(settings.directory.as_deref(), progress)
}

pub(crate) fn models_with_directory(
    directory: Option<&Path>,
    progress: &dyn Fn(InferenceStatus),
) -> Result<PathBuf> {
    if let Some(path) = directory {
        return local_models(path);
    }
    if let Some(path) = std::env::var_os("IRODORU_MODELS") {
        return local_models(Path::new(&path));
    }
    if cfg!(debug_assertions) {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/models");
        if path.join("anime/manifest.json").is_file() {
            return local_models(&path);
        }
    }
    let directories = directories::ProjectDirs::from("io.github", "kanade-k-1228", "irodoru")
        .context("モデルの保存フォルダが見つかりません")?;
    super::model_resources::download(
        include_bytes!("../../resources/model-distribution.json"),
        &directories.data_local_dir().join("models"),
        progress,
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn development_override_requires_both_model_directories() {
        let root = tempfile::tempdir().unwrap();
        assert!(super::local_models(root.path()).is_err());
        std::fs::create_dir_all(root.path().join("anime")).unwrap();
        std::fs::write(root.path().join("anime/manifest.json"), b"{}").unwrap();
        assert!(super::local_models(root.path()).is_err());
        std::fs::create_dir_all(root.path().join("codec")).unwrap();
        std::fs::write(root.path().join("codec/codec.json"), b"{}").unwrap();
        assert_eq!(
            super::local_models(root.path()).unwrap(),
            root.path().canonicalize().unwrap()
        );
    }
}
