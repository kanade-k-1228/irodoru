//! Own the active session, archive reservation and autosave worker together.
use super::{ProjectStatus, Saver, Session, SharedProject, ownership::WindowProject, status};
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
        self.flush()?;
        let session = self.ownership.open(path, create)?;
        *self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("project lock poisoned"))? = Some(session);
        if create {
            self.saver.changed()?;
        }
        Ok(())
    }

    pub fn read<T>(&self, read: impl FnOnce(&Session) -> Result<T>) -> Result<T> {
        let guard = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("project lock poisoned"))?;
        read(
            guard
                .as_ref()
                .context("プロジェクトを作成または開いてください")?,
        )
    }

    pub fn change<T>(&self, change: impl FnOnce(&Session) -> Result<T>) -> Result<T> {
        let mut guard = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("project lock poisoned"))?;
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
