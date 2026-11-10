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

pub(crate) fn models_with_directory(
    directory: Option<&Path>,
    progress: &dyn Fn(InferenceStatus),
) -> Result<PathBuf> {
    let environment = std::env::var_os("IRODORU_MODELS").map(PathBuf::from);
    let development = cfg!(debug_assertions)
        .then(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/models"));
    if let Some(path) = resolve_local(directory, environment.as_deref(), development.as_deref())? {
        return Ok(path);
    }
    let directories =
        crate::files::project_dirs().context("モデルの保存フォルダが見つかりません")?;
    super::model_resources::download(
        include_bytes!("../../resources/model-distribution.json"),
        &directories.data_local_dir().join("models"),
        progress,
    )
}

fn resolve_local(
    directory: Option<&Path>,
    environment: Option<&Path>,
    development: Option<&Path>,
) -> Result<Option<PathBuf>> {
    if let Some(path) = directory.or(environment) {
        return local_models(path).map(Some);
    }
    development
        .filter(|path| path.join("anime/manifest.json").is_file())
        .map(local_models)
        .transpose()
}
