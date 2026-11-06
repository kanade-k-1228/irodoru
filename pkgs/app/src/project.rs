//! Project sessions and an event-driven, single-writer ZIP saver.
use std::{
    fs::File,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::library::{Library, new_id};

#[derive(Clone, Debug, Serialize)]
pub struct ProjectStatus {
    pub id: Option<String>,
    pub name: Option<String>,
    pub path: Option<String>,
    pub state: String,
    pub error: Option<String>,
}

pub struct Session {
    pub id: String,
    pub library: Library,
    pub previews: Library,
    pub path: PathBuf,
    revision: u64,
    saved: u64,
    error: Option<String>,
    // Own the working files until the session and its snapshots have gone away.
    _directory: tempfile::TempDir,
}
impl Session {
    pub fn create(path: PathBuf) -> Result<Self> {
        let directory = tempfile::Builder::new()
            .prefix("irodoru-project-")
            .tempdir()?;
        let library = Library::open(directory.path())?;
        library.save_editor(&Default::default())?;
        let previews = Library::open(&directory.path().join(".previews"))?;
        Ok(Self {
            id: new_id(),
            library,
            previews,
            path,
            revision: 1,
            saved: 0,
            error: None,
            _directory: directory,
        })
    }
    pub fn open(path: PathBuf) -> Result<Self> {
        let session = Self::create(path.clone())?;
        extract(File::open(path)?, session.library.root())?;
        session.validate()?;
        Ok(Self {
            saved: 1,
            ..session
        })
    }
    fn validate(&self) -> Result<()> {
        let editor = self.library.editor()?;
        let characters = self.library.characters()?;
        let takes = self.library.takes()?;
        let mut rows = std::collections::HashSet::new();
        for row in &editor.rows {
            if row.id.is_empty() || row.id.len() > 64 || !rows.insert(&row.id) {
                bail!("台本の行 ID が不正です");
            }
            if !row.character_id.is_empty() && !characters.iter().any(|c| c.id == row.character_id)
            {
                bail!("キャラクタが見つかりません");
            }
            if let Some(id) = &row.take_id
                && !takes
                    .iter()
                    .any(|take| &take.id == id && take.row_id == row.id)
            {
                bail!("生成音声が見つかりません");
            }
        }
        let mut generated = std::collections::HashSet::new();
        for take in &takes {
            if !rows.contains(&take.row_id) || !generated.insert(&take.row_id) {
                bail!("生成音声と台本が一致しません");
            }
            crate::generation::validate_wav(&self.library.audio(&take.id)?)?;
        }
        for character in characters {
            if let Some(voice) = character.voice {
                crate::generation::validate_wav(&self.library.voice(&voice)?)?;
            }
        }
        Ok(())
    }
    pub fn changed(&mut self) {
        self.revision += 1;
        self.error = None;
    }
    fn status(&self) -> ProjectStatus {
        ProjectStatus {
            id: Some(self.id.clone()),
            name: Some(
                self.path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            ),
            path: Some(self.path.to_string_lossy().into_owned()),
            state: if self.error.is_some() {
                "error"
            } else if self.revision != self.saved {
                "saving"
            } else {
                "saved"
            }
            .into(),
            error: self.error.clone(),
        }
    }
}

pub type SharedProject = Arc<Mutex<Option<Session>>>;
type Report = Arc<dyn Fn(ProjectStatus) + Send + Sync>;

enum Message {
    Changed,
    Flush(mpsc::Sender<Result<(), String>>),
    Stop,
}

pub struct Saver {
    sender: mpsc::Sender<Message>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Saver {
    pub fn start(
        project: SharedProject,
        report: impl Fn(ProjectStatus) + Send + Sync + 'static,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let report: Report = Arc::new(report);
        let thread = std::thread::Builder::new()
            .name("project-save".into())
            .spawn(move || {
                while let Ok(message) = receiver.recv() {
                    let mut waiters = Vec::new();
                    match message {
                        Message::Stop => break,
                        Message::Flush(reply) => waiters.push(reply),
                        Message::Changed => {
                            // Debounce action bursts, but keep typing from postponing saving forever.
                            let deadline = Instant::now() + Duration::from_secs(1);
                            loop {
                                let delay = Duration::from_millis(200)
                                    .min(deadline.saturating_duration_since(Instant::now()));
                                match receiver.recv_timeout(delay) {
                                    Ok(Message::Changed) if Instant::now() < deadline => {}
                                    Ok(Message::Changed) => break,
                                    Ok(Message::Flush(reply)) => {
                                        waiters.push(reply);
                                        break;
                                    }
                                    Ok(Message::Stop) => return,
                                    Err(_) => break,
                                }
                            }
                        }
                    }
                    let result = save_current(&project, &report).map_err(|e| format!("{e:#}"));
                    for reply in waiters {
                        let _ = reply.send(result.clone());
                    }
                }
            })?;
        Ok(Self {
            sender,
            thread: Some(thread),
        })
    }
    pub fn changed(&self) -> Result<()> {
        self.sender
            .send(Message::Changed)
            .context("保存ワーカーが終了しています")
    }
    pub fn flush_sender(&self) -> Result<mpsc::Receiver<Result<(), String>>> {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(Message::Flush(sender))
            .context("保存ワーカーが終了しています")?;
        Ok(receiver)
    }
    pub fn flush(&self) -> Result<()> {
        self.flush_sender()?
            .recv()
            .context("保存ワーカーが終了しています")?
            .map_err(anyhow::Error::msg)
    }
}
impl Drop for Saver {
    fn drop(&mut self) {
        let _ = self.sender.send(Message::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn status(project: &SharedProject) -> Result<ProjectStatus> {
    Ok(project
        .lock()
        .map_err(|_| anyhow::anyhow!("project lock poisoned"))?
        .as_ref()
        .map(Session::status)
        .unwrap_or(ProjectStatus {
            id: None,
            name: None,
            path: None,
            state: "closed".into(),
            error: None,
        }))
}

struct Snapshot {
    id: String,
    revision: u64,
    path: PathBuf,
    files: Vec<(String, PathBuf)>,
    _directory: tempfile::TempDir,
}
fn snapshot(session: &Session) -> Result<Snapshot> {
    fn visit(
        root: &Path,
        path: &Path,
        staging: &Path,
        files: &mut Vec<(String, PathBuf)>,
    ) -> Result<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                visit(root, &path, staging, files)?;
            } else if entry.file_type()?.is_file() {
                let relative = path.strip_prefix(root)?;
                let target = staging.join(relative);
                std::fs::create_dir_all(target.parent().unwrap())?;
                // Working files are replaced atomically, never modified in place. Hard links
                // pin their current contents without holding Windows handles to editable paths.
                // Copy on filesystems without hard-link support.
                if std::fs::hard_link(&path, &target).is_err() {
                    std::fs::copy(&path, &target)?;
                }
                files.push((relative.to_string_lossy().replace('\\', "/"), target));
            } else {
                bail!("プロジェクトに通常ファイル以外が含まれています");
            }
        }
        Ok(())
    }
    let directory = tempfile::Builder::new()
        .prefix(".snapshot-")
        .tempdir_in(session.library.root())?;
    let mut files = Vec::new();
    visit(
        session.library.root(),
        session.library.root(),
        directory.path(),
        &mut files,
    )?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(Snapshot {
        id: session.id.clone(),
        revision: session.revision,
        path: session.path.clone(),
        files,
        _directory: directory,
    })
}
fn save_current(project: &SharedProject, report: &Report) -> Result<()> {
    let snapshot = {
        let guard = project
            .lock()
            .map_err(|_| anyhow::anyhow!("project lock poisoned"))?;
        let Some(session) = guard.as_ref() else {
            return Ok(());
        };
        if session.revision == session.saved {
            return Ok(());
        }
        report(session.status());
        snapshot(session)
    };
    let result = snapshot
        .as_ref()
        .map_err(|e| anyhow::anyhow!("{e:#}"))
        .and_then(write_archive);
    let mut guard = project
        .lock()
        .map_err(|_| anyhow::anyhow!("project lock poisoned"))?;
    if let Some(session) = guard.as_mut() {
        if let Ok(snapshot) = &snapshot {
            if session.id != snapshot.id {
                return result;
            }
            if result.is_ok() {
                session.saved = snapshot.revision;
            }
        }
        session.error = result.as_ref().err().map(|e| format!("{e:#}"));
        report(session.status());
    }
    result
}
fn write_archive(snapshot: &Snapshot) -> Result<()> {
    let parent = snapshot.path.parent().context("保存先が不正です")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    let mut zip = ZipWriter::new(temp.as_file_mut());
    for (name, source) in &snapshot.files {
        let method = if name.ends_with(".yaml") {
            zip::CompressionMethod::Deflated
        } else {
            zip::CompressionMethod::Stored
        };
        zip.start_file(
            name,
            SimpleFileOptions::default().compression_method(method),
        )?;
        std::io::copy(&mut File::open(source)?, &mut zip)?;
    }
    zip.finish()?.sync_all()?;
    temp.persist(&snapshot.path).map_err(|e| e.error)?;
    Ok(())
}

fn extract(file: File, root: &Path) -> Result<()> {
    let mut zip = ZipArchive::new(file)?;
    if zip.len() > 100_000 {
        bail!("プロジェクトのファイル数が多すぎます");
    }
    let mut seen = std::collections::HashSet::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name()?.into_owned();
        let path = Path::new(&name);
        if name.contains(['\\', ':'])
            || path
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            bail!("ZIP 内のパスが不正です");
        }
        if entry.is_dir() {
            continue;
        }
        if !seen.insert(name.clone()) {
            bail!("ZIP 内のファイルが重複しています");
        }
        let parts: Vec<_> = name.split('/').collect();
        let allowed = match parts.as_slice() {
            ["project.yaml"] => true,
            [
                "characters",
                id,
                "character.yaml" | "reference.wav" | "icon.png",
            ] => crate::library::valid_id(id),
            ["voices", id, "audio.wav" | "meta.yaml"] => crate::library::valid_id(id),
            ["characters", id, "previews", "audio.wav" | "meta.yaml"] => {
                crate::library::valid_id(id)
            }
            _ => false,
        };
        if !allowed {
            bail!("未対応のプロジェクトファイル: {name}");
        }
        const LIMIT: u64 = 4 * 1024 * 1024 * 1024;
        let limit = if name.ends_with(".yaml") || name.ends_with(".png") {
            4 * 1024 * 1024
        } else {
            LIMIT - total
        };
        if entry.size() > limit || entry.size() > LIMIT - total {
            bail!("プロジェクトが大きすぎます");
        }
        let destination = root.join(path);
        std::fs::create_dir_all(destination.parent().unwrap())?;
        let mut output = File::create(destination)?;
        let count = std::io::copy(
            &mut entry.by_ref().take(limit.min(LIMIT - total) + 1),
            &mut output,
        )?;
        total += count;
        if count > limit || total > LIMIT {
            bail!("プロジェクトが大きすぎます");
        }
        output.flush()?;
    }
    if !seen.contains("project.yaml") {
        bail!("project.yaml がありません");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{library::CharacterInput, types::ScriptRow};
    fn fixture(path: PathBuf) -> Session {
        let session = Session::create(path).unwrap();
        let mut wav = std::io::Cursor::new(Vec::new());
        let mut writer = hound::WavWriter::new(
            &mut wav,
            hound::WavSpec {
                channels: 1,
                sample_rate: 24000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..24 {
            writer.write_sample(100i16).unwrap();
        }
        writer.finalize().unwrap();
        session
            .library
            .save_voice("source", &wav.into_inner())
            .unwrap();
        let character = session
            .library
            .create_character(CharacterInput {
                name: "さくら".into(),
                description: "落ち着いた声".into(),
                voice: Some("source".into()),
                color: None,
                sample_text: None,
                icon: None,
            })
            .unwrap();
        let mut editor = session.library.editor().unwrap();
        editor.rows.push(ScriptRow {
            id: "line".into(),
            character_id: character.id,
            text: "最初".into(),
            take_id: None,
            prompt: None,
            settings: None,
        });
        session.library.save_editor(&editor).unwrap();
        session
    }
    #[test]
    fn archive_round_trip_is_portable_and_has_no_shared_or_draft_files() {
        let dir = tempfile::tempdir().unwrap();
        let session = fixture(dir.path().join("first.zip"));
        write_archive(&snapshot(&session).unwrap()).unwrap();
        let moved = dir.path().join("renamed.zip");
        std::fs::rename(&session.path, &moved).unwrap();
        drop(session);
        let reopened = Session::open(moved.clone()).unwrap();
        assert_eq!(reopened.library.editor().unwrap().rows[0].text, "最初");
        let character = reopened.library.characters().unwrap().remove(0);
        assert!(
            !reopened
                .library
                .voice(character.voice.as_ref().unwrap())
                .unwrap()
                .is_empty()
        );
        let zip = ZipArchive::new(File::open(moved).unwrap()).unwrap();
        let names: Vec<_> = zip
            .file_names()
            .collect::<zip::result::ZipResult<_>>()
            .unwrap();
        assert!(names.iter().any(|name| name == "project.yaml"));
        assert!(names.iter().any(|name| name.ends_with("/character.yaml")));
        assert!(names.iter().all(|name| !name.starts_with('.')));
    }
    #[test]
    fn only_latest_audio_and_saved_character_preview_survive_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let session = fixture(dir.path().join("project.zip"));
        let character = session.library.characters().unwrap().remove(0);
        let wav = session
            .library
            .voice(character.voice.as_ref().unwrap())
            .unwrap();
        let take = crate::library::Take {
            filename: None,
            id: new_id(),
            row_id: "line".into(),
            character_id: Some(character.id.clone()),
            character_name: character.name,
            description: character.description,
            additional_prompt: String::new(),
            voice: character.voice,
            text: "最初".into(),
            model: "anime".into(),
            seed: 42,
            num_steps: 1,
            elapsed_ms: 10,
            audio_seconds: 0.001,
            created_at: 1,
        };
        session.library.save_take(&take, &wav).unwrap();
        let stale_editor = session.library.editor().unwrap();
        let latest = crate::library::Take {
            id: new_id(),
            created_at: 2,
            ..take.clone()
        };
        session.library.save_take(&latest, &wav).unwrap();
        session.library.save_editor(&stale_editor).unwrap();
        session.previews.save_take(&take, &wav).unwrap();
        session
            .library
            .save_character_preview(&character.id, &session.previews, &take.id)
            .unwrap();
        write_archive(&snapshot(&session).unwrap()).unwrap();
        let reopened = Session::open(session.path.clone()).unwrap();
        assert_eq!(reopened.library.takes().unwrap().len(), 1);
        assert_eq!(
            reopened.library.editor().unwrap().rows[0]
                .take_id
                .as_deref(),
            Some(latest.id.as_str())
        );
        assert!(reopened.library.audio(&take.id).is_err());
        assert_eq!(reopened.library.audio(&latest.id).unwrap(), wav);
        assert_eq!(
            std::fs::read(
                reopened
                    .library
                    .character_preview_path(&character.id)
                    .unwrap()
                    .unwrap()
            )
            .unwrap(),
            wav
        );
        reopened.library.save_editor(&Default::default()).unwrap();
        assert!(reopened.library.takes().unwrap().is_empty());
    }
    #[test]
    #[cfg(unix)]
    fn failed_archive_write_leaves_the_previous_file_intact() {
        let dir = tempfile::tempdir().unwrap();
        let session = fixture(dir.path().join("project.zip"));
        write_archive(&snapshot(&session).unwrap()).unwrap();
        let bytes = std::fs::read(&session.path).unwrap();
        let mut broken = snapshot(&session).unwrap();
        // Reading a directory as a regular file fails after ZIP construction has started.
        #[cfg(unix)]
        {
            broken
                .files
                .push(("broken.wav".into(), dir.path().to_path_buf()));
            assert!(write_archive(&broken).is_err());
            assert_eq!(std::fs::read(&session.path).unwrap(), bytes);
        }
    }
    #[test]
    fn snapshot_is_immutable_while_editing_and_flush_saves_the_latest_revision() {
        let dir = tempfile::tempdir().unwrap();
        let mut session = fixture(dir.path().join("project.zip"));
        let old = snapshot(&session).unwrap();
        let pinned = &old
            .files
            .iter()
            .find(|(name, _)| name == "project.yaml")
            .unwrap()
            .1;
        let mut snapshot_reader = File::open(pinned).unwrap();
        let mut editor = session.library.editor().unwrap();
        editor.rows[0].text = "更新".into();
        session.library.save_editor(&editor).unwrap();
        session.changed();
        let mut captured = String::new();
        snapshot_reader.read_to_string(&mut captured).unwrap();
        assert!(captured.contains("最初"));
        drop(snapshot_reader);
        write_archive(&old).unwrap();
        assert_eq!(
            Session::open(session.path.clone())
                .unwrap()
                .library
                .editor()
                .unwrap()
                .rows[0]
                .text,
            "最初"
        );
        let path = session.path.clone();
        let project = Arc::new(Mutex::new(Some(session)));
        let saver = Saver::start(project.clone(), |_| {}).unwrap();
        for _ in 0..10 {
            saver.changed().unwrap();
        }
        saver.flush().unwrap();
        assert_eq!(status(&project).unwrap().state, "saved");
        assert_eq!(
            Session::open(path).unwrap().library.editor().unwrap().rows[0].text,
            "更新"
        );
    }
    #[test]
    fn save_failure_keeps_previous_archive_and_retry_recovers() {
        let dir = tempfile::tempdir().unwrap();
        let session = fixture(dir.path().join("project.zip"));
        let original = snapshot(&session).unwrap();
        write_archive(&original).unwrap();
        let before = std::fs::read(&original.path).unwrap();
        let project = Arc::new(Mutex::new(Some(session)));
        let saver = Saver::start(project.clone(), |_| {}).unwrap();
        {
            let mut guard = project.lock().unwrap();
            let session = guard.as_mut().unwrap();
            session.path = dir.path().join("missing/project.zip");
            session.changed();
        }
        assert!(saver.flush().is_err());
        assert_eq!(status(&project).unwrap().state, "error");
        assert_eq!(std::fs::read(&original.path).unwrap(), before);
        project.lock().unwrap().as_mut().unwrap().path = original.path;
        saver.flush().unwrap();
        assert_eq!(status(&project).unwrap().state, "saved");
    }
    #[test]
    fn invalid_archives_do_not_escape_the_project_or_silently_open_empty() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "../outside",
            "/outside",
            "characters/../../outside",
            "characters\\outside",
            "unrelated.txt",
        ] {
            let path = dir.path().join("bad.zip");
            let mut zip = ZipWriter::new(File::create(&path).unwrap());
            zip.start_file(name, SimpleFileOptions::default()).unwrap();
            zip.write_all(b"bad").unwrap();
            zip.finish().unwrap();
            assert!(Session::open(path).is_err(), "{name}");
        }
        let path = dir.path().join("empty.zip");
        ZipWriter::new(File::create(&path).unwrap())
            .finish()
            .unwrap();
        assert!(Session::open(path).is_err());
        assert!(!dir.path().join("outside").exists());
    }
    #[test]
    fn flush_finishes_the_old_project_before_switching() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.zip");
        let b = dir.path().join("b.zip");
        let project = Arc::new(Mutex::new(Some(fixture(a.clone()))));
        let saver = Saver::start(project.clone(), |_| {}).unwrap();
        saver.changed().unwrap();
        saver.flush().unwrap();
        *project.lock().unwrap() = Some(Session::create(b.clone()).unwrap());
        saver.changed().unwrap();
        saver.flush().unwrap();
        assert_eq!(
            Session::open(a)
                .unwrap()
                .library
                .editor()
                .unwrap()
                .rows
                .len(),
            1
        );
        assert!(
            Session::open(b)
                .unwrap()
                .library
                .characters()
                .unwrap()
                .is_empty()
        );
    }
}
