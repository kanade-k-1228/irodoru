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
    let directories = directories::ProjectDirs::from("io.github", "kanade-k-1228", "irodoru")
        .context("モデルの保存フォルダが見つかりません")?;
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
            super::models_with_directory(Some(root.path()), &|_| panic!(
                "explicit models must not download"
            ))
            .unwrap(),
            root.path().canonicalize().unwrap()
        );
    }
}

#[cfg(test)]
mod precedence_tests {
    use super::*;
    #[test]
    fn explicit_then_environment_then_development_without_silent_fallback() {
        let root = tempfile::tempdir().unwrap();
        let paths = ["chosen", "environment", "development"].map(|name| root.path().join(name));
        for path in &paths {
            std::fs::create_dir_all(path.join("anime")).unwrap();
            std::fs::create_dir_all(path.join("codec")).unwrap();
            std::fs::write(path.join("anime/manifest.json"), "{}").unwrap();
            std::fs::write(path.join("codec/codec.json"), "{}").unwrap();
        }
        for (selected, environment, expected) in [
            (
                Some(paths[0].as_path()),
                Some(paths[1].as_path()),
                &paths[0],
            ),
            (None, Some(paths[1].as_path()), &paths[1]),
            (None, None, &paths[2]),
        ] {
            assert_eq!(
                resolve_local(selected, environment, Some(&paths[2])).unwrap(),
                Some(expected.canonicalize().unwrap())
            );
        }
        let missing = root.path().join("missing");
        assert!(resolve_local(Some(&missing), Some(&paths[1]), Some(&paths[2])).is_err());
        assert!(resolve_local(None, Some(&missing), Some(&paths[2])).is_err());
        assert_eq!(resolve_local(None, None, Some(&missing)).unwrap(), None);
    }
}
