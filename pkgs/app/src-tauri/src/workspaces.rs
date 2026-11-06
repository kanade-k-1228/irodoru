//! Project state and save workers owned by individual application windows.
use crate::{
    generation::PreparedBatch,
    library::Library,
    project::{ProjectStatus, Saver, Session, SharedProject},
};
use anyhow::{Context, Result, bail};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

pub struct Workspace {
    pub project: SharedProject,
    pub saver: Saver,
    pub switching: AtomicBool,
    pub previews_running: AtomicUsize,
    pub batch: Mutex<Option<PreparedBatch>>,
}
type CommandResult<T> = Result<T, String>;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
impl Workspace {
    pub fn read<T>(&self, read: impl FnOnce(&Session) -> anyhow::Result<T>) -> CommandResult<T> {
        let guard = self.project.lock().map_err(err)?;
        if self.switching.load(Ordering::SeqCst) {
            return Err("プロジェクトを切り替え中です".into());
        }
        read(
            guard
                .as_ref()
                .ok_or("プロジェクトを作成または開いてください")?,
        )
        .map_err(err)
    }
    pub fn change<T>(
        &self,
        change: impl FnOnce(&Library) -> Result<T, crate::error::AppError>,
    ) -> CommandResult<T> {
        self.change_session(|session| change(&session.library))
    }
    pub fn change_session<T>(
        &self,
        change: impl FnOnce(&Session) -> Result<T, crate::error::AppError>,
    ) -> CommandResult<T> {
        let mut guard = self.project.lock().map_err(err)?;
        if self.switching.load(Ordering::SeqCst) {
            return Err("プロジェクトを切り替え中です".into());
        }
        let session = guard
            .as_mut()
            .ok_or("プロジェクトを作成または開いてください")?;
        let result = change(session).map_err(err)?;
        session.changed();
        self.saver.changed().map_err(err)?;
        Ok(result)
    }
}

#[derive(Default)]
pub struct Workspaces {
    windows: Mutex<HashMap<String, Arc<Workspace>>>,
    paths: Mutex<HashMap<PathBuf, String>>,
}
impl Workspaces {
    pub fn insert(
        &self,
        label: &str,
        report: impl Fn(ProjectStatus) + Send + Sync + 'static,
    ) -> Result<()> {
        let mut windows = self.windows.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        if windows.contains_key(label) {
            bail!("ウィンドウは既に存在します");
        }
        let project = Arc::new(Mutex::new(None));
        let saver = Saver::start(project.clone(), report)?;
        windows.insert(
            label.to_owned(),
            Arc::new(Workspace {
                project,
                saver,
                switching: AtomicBool::new(false),
                previews_running: AtomicUsize::new(0),
                batch: Mutex::new(None),
            }),
        );
        Ok(())
    }
    pub fn get(&self, label: &str) -> Result<Arc<Workspace>> {
        self.windows
            .lock()
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .get(label)
            .cloned()
            .context("ウィンドウが見つかりません")
    }
    pub fn remove(&self, label: &str) {
        let removed = self.windows.lock().unwrap().remove(label);
        self.paths.lock().unwrap().retain(|_, owner| owner != label);
        drop(removed);
    }
    pub fn reserve<'a>(&'a self, label: &str, path: &Path) -> Result<ProjectPath<'a>> {
        // Resolve aliases before taking ownership, including new archives in symlinked folders.
        let path = if path.exists() {
            path.canonicalize()?
        } else {
            path.parent()
                .context("保存先が不正です")?
                .canonicalize()?
                .join(path.file_name().context("保存先が不正です")?)
        };
        let mut paths = self.paths.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        let already_owned = match paths.get(&path) {
            Some(owner) if owner != label => {
                bail!("このプロジェクトは別のウィンドウで開いています")
            }
            Some(_) => true,
            None => false,
        };
        paths.insert(path.clone(), label.to_owned());
        Ok(ProjectPath {
            workspaces: self,
            path,
            label: label.to_owned(),
            already_owned,
            committed: false,
        })
    }
}

pub struct ProjectPath<'a> {
    workspaces: &'a Workspaces,
    pub path: PathBuf,
    label: String,
    already_owned: bool,
    committed: bool,
}
impl ProjectPath<'_> {
    pub fn commit(mut self) {
        self.workspaces
            .paths
            .lock()
            .unwrap()
            .retain(|path, owner| owner != &self.label || path == &self.path);
        self.committed = true;
    }
}
impl Drop for ProjectPath<'_> {
    fn drop(&mut self) {
        if !self.committed && !self.already_owned {
            self.workspaces.paths.lock().unwrap().remove(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{EditorState, ScriptRow};

    #[test]
    fn windows_save_independent_projects_and_report_only_their_own_status() {
        let dir = tempfile::tempdir().unwrap();
        let workspaces = Workspaces::default();
        let reports = Arc::new(Mutex::new(Vec::new()));
        for label in ["main", "project-second"] {
            let reports = reports.clone();
            workspaces
                .insert(label, move |status| {
                    reports.lock().unwrap().push((label, status));
                })
                .unwrap();
            let workspace = workspaces.get(label).unwrap();
            let path = dir.path().join(format!("{label}.zip"));
            let reservation = workspaces.reserve(label, &path).unwrap();
            *workspace.project.lock().unwrap() =
                Some(Session::create(reservation.path.clone()).unwrap());
            reservation.commit();
            workspace
                .change(|library| {
                    library.save_editor(&EditorState {
                        rows: vec![ScriptRow {
                            id: "same-row-id".into(),
                            text: label.into(),
                            character_id: String::new(),
                            take_id: None,
                            prompt: None,
                            settings: None,
                        }],
                        ..Default::default()
                    })
                })
                .unwrap();
        }
        for label in ["main", "project-second"] {
            workspaces.get(label).unwrap().saver.flush().unwrap();
            let session = Session::open(dir.path().join(format!("{label}.zip"))).unwrap();
            assert_eq!(session.library.editor().unwrap().rows[0].text, label);
        }
        for (label, status) in reports.lock().unwrap().iter() {
            assert_eq!(
                status.name.as_deref(),
                Some(format!("{label}.zip").as_str())
            );
        }
        workspaces.remove("main");
        assert!(workspaces.get("main").is_err());
        assert!(workspaces.get("project-second").is_ok());
    }

    #[test]
    fn archive_ownership_survives_failed_opens_and_releases_on_switch_or_close() {
        let dir = tempfile::tempdir().unwrap();
        let workspaces = Workspaces::default();
        let first = dir.path().join("first.zip");
        let second = dir.path().join("second.zip");
        workspaces.reserve("main", &first).unwrap().commit();
        assert!(workspaces.reserve("other", &first).is_err());
        // Reopening our current archive then failing must retain its ownership.
        drop(workspaces.reserve("main", &first).unwrap());
        assert!(workspaces.reserve("other", &first).is_err());
        // A failed attempt to switch must release only the proposed archive.
        {
            let _pending = workspaces.reserve("main", &second).unwrap();
            assert!(workspaces.reserve("other", &second).is_err());
        }
        assert!(workspaces.reserve("other", &second).is_ok());
        assert!(workspaces.reserve("other", &first).is_err());
        workspaces.reserve("main", &second).unwrap().commit();
        assert!(workspaces.reserve("other", &first).is_ok());
        assert!(workspaces.reserve("other", &second).is_err());
        workspaces.remove("main");
        assert!(workspaces.reserve("other", &second).is_ok());
    }

    #[test]
    fn simultaneous_open_requests_cannot_claim_the_same_archive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shared.zip");
        let workspaces = Workspaces::default();
        let barrier = std::sync::Barrier::new(2);
        let results = std::thread::scope(|scope| {
            let claim = |label| {
                barrier.wait();
                workspaces
                    .reserve(label, &path)
                    .map(|reservation| reservation.commit())
                    .is_ok()
            };
            let first = scope.spawn(move || claim("main"));
            let second = scope.spawn(move || claim("other"));
            [first.join().unwrap(), second.join().unwrap()]
        });
        assert_eq!(results.into_iter().filter(|won| *won).count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn aliases_cannot_bypass_archive_ownership() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("project.zip");
        std::fs::write(&archive, []).unwrap();
        let alias = dir.path().join("alias.zip");
        std::os::unix::fs::symlink(&archive, &alias).unwrap();
        let workspaces = Workspaces::default();
        workspaces.reserve("main", &archive).unwrap().commit();
        assert!(workspaces.reserve("other", &alias).is_err());
    }
}
