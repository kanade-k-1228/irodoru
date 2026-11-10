//! Own the active session, archive reservation and autosave worker together.
use super::{ProjectStatus, Saver, Session, SharedProject, lock, ownership::WindowProject, status};
use anyhow::{Context, Result};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

pub struct Project {
    session: SharedProject,
    // Stop the saver before releasing the archive's ownership.
    saver: Saver,
    ownership: WindowProject,
}

impl Project {
    pub fn new(report: impl Fn(ProjectStatus) + Send + Sync + 'static) -> Result<Self> {
        let session = Arc::new(Mutex::new(None));
        let saver = Saver::start(session.clone(), report)?;
        Ok(Self {
            session,
            saver,
            ownership: WindowProject::default(),
        })
    }

    pub fn open(&mut self, path: &Path, create: bool) -> Result<()> {
        super::validate_path(path)?;
        self.flush()?;
        let session = self.ownership.open(path, create)?;
        *lock(&self.session)? = Some(session);
        if create {
            self.saver.changed()?;
        }
        Ok(())
    }

    pub fn create_from_charas(&mut self, path: &Path) -> Result<()> {
        super::validate_path(path)?;
        self.flush()?;
        // Reserve separately: the current project remains open if copying or saving fails.
        let mut ownership = WindowProject::default();
        let mut session = ownership.open(path, true)?;
        self.read(|source| {
            let ids = source
                .library
                .charas()?
                .into_iter()
                .map(|c| c.id)
                .collect::<Vec<_>>();
            source.library.copy_charas_to(&session.library, &ids)?;
            session.library.save_editor(&Default::default())
        })?;
        super::archive::write_archive(&super::archive::snapshot(&session)?)?;
        session.saved = session.revision;
        *lock(&self.session)? = Some(session);
        self.ownership = ownership;
        Ok(())
    }

    pub fn read<T>(&self, read: impl FnOnce(&Session) -> Result<T>) -> Result<T> {
        let guard = lock(&self.session)?;
        read(
            guard
                .as_ref()
                .context("プロジェクトを作成または開いてください")?,
        )
    }

    pub fn change<T>(&self, change: impl FnOnce(&Session) -> Result<T>) -> Result<T> {
        let mut guard = lock(&self.session)?;
        let session = guard
            .as_mut()
            .context("プロジェクトを作成または開いてください")?;
        let value = change(session)?;
        session.changed();
        drop(guard);
        self.saver.changed()?;
        Ok(value)
    }

    pub fn status(&self) -> Result<ProjectStatus> {
        status(&self.session)
    }

    pub fn flush(&self) -> Result<()> {
        self.saver.flush()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}
