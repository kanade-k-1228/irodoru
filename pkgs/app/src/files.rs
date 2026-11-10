//! Atomic asset replacement for project persistence.
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
