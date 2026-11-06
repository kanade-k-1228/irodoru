use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

pub fn candidates(executable: &Path) -> Vec<PathBuf> {
    let directory = executable.parent().unwrap_or(Path::new("."));
    vec![
        directory.join("models"),
        directory.join("../Resources/models"),
        directory.join("../lib/irodoru/models"),
        directory.join("../lib/irodoru-desktop/models"),
    ]
}
pub fn models() -> Result<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = std::env::var_os("IRODORU_MODELS") {
        paths.push(PathBuf::from(path));
    }
    if cfg!(debug_assertions) {
        paths.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/models"));
    }
    paths.extend(candidates(&std::env::current_exe()?));
    for path in &paths {
        if path.join("anime/manifest.json").is_file() && path.join("codec/codec.json").is_file() {
            return Ok(path.canonicalize()?);
        }
    }
    bail!(
        "モデルが見つかりません。cargo xtask models prepare を実行してください。検索先: {paths:?}"
    )
}
#[cfg(test)]
mod tests {
    #[test]
    fn resource_paths_are_relative_to_executable() {
        let p = super::candidates(std::path::Path::new("/opt/irodoru/bin/irodoru"));
        assert_eq!(p[0], std::path::Path::new("/opt/irodoru/bin/models"));
        assert!(p.iter().any(|p| p.ends_with("../Resources/models")));
        assert!(p.iter().any(|p| p.ends_with("../lib/irodoru/models")));
    }
}
