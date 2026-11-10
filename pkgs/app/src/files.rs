//! Per-user directories and atomic file replacement.
use anyhow::Result;
use std::path::Path as FsPath;
pub(crate) fn atomic_bytes(path: &FsPath, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path)?;
    Ok(())
}
/// The per-user configuration and data directories of this application.
pub(crate) fn project_dirs() -> Option<directories::ProjectDirs> {
    directories::ProjectDirs::from("io.github", "kanade-k-1228", "irodoru")
}
