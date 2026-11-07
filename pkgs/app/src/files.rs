//! Atomic asset replacement for project persistence.
use crate::domain::AppError;
use anyhow::Result;
use std::path::Path as FsPath;
pub(crate) fn atomic_bytes(path: &FsPath, bytes: &[u8]) -> Result<(), AppError> {
    use std::io::Write;
    let mut temp =
        tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(AppError::internal)?;
    temp.write_all(bytes).map_err(AppError::internal)?;
    temp.as_file().sync_all().map_err(AppError::internal)?;
    temp.persist(path).map_err(AppError::internal)?;
    Ok(())
}
