//! Ordered application commands. All storage and synthesis work runs off the UI thread.
use crate::{
    domain::TakeRequest,
    domain::new_id,
    project::{self, Saver, Session, SharedProject},
    synthesis::Inference,
    synthesis::generation,
};
use anyhow::{Context, Result};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

use super::protocol::{Command, Event, Snapshot};
pub(super) struct Worker {
    project: SharedProject,
    saver: Saver,
    inference: Arc<Inference>,
    stop: Arc<AtomicBool>,
    events: mpsc::Sender<Event>,
    editor_error: Option<String>,
    window_project: crate::project::WindowProject,
}
impl Worker {
    pub(super) fn run(
        receiver: mpsc::Receiver<Command>,
        events: mpsc::Sender<Event>,
        inference: Arc<Inference>,
        stop: Arc<AtomicBool>,
    ) {
        let project: SharedProject = Arc::new(Mutex::new(None));
        let report = events.clone();
        let saver = match Saver::start(project.clone(), move |s| {
            let _ = report.send(Event::Status(s));
        }) {
            Ok(s) => s,
            Err(e) => {
                let _ = events.send(Event::Error(e.to_string()));
                return;
            }
        };
        let mut worker = Worker {
            project,
            saver,
            inference,
            stop,
            editor_error: None,
            window_project: Default::default(),
            events,
        };
        while let Ok(command) = receiver.recv() {
            let passive = matches!(command, Command::SaveEditor(_) | Command::RemovePreview(_));
            if let Err(e) = worker.execute(command) {
                let _ = worker.events.send(Event::Error(format!("{e:#}")));
            }
            if !passive {
                let _ = worker.events.send(Event::Done);
            }
        }
        let _ = worker.saver.flush();
    }

    fn read<T>(&self, f: impl FnOnce(&Session) -> Result<T>) -> Result<T> {
        let guard = self
            .project
            .lock()
            .map_err(|_| anyhow::anyhow!("project lock poisoned"))?;
        f(guard
            .as_ref()
            .context("プロジェクトを作成または開いてください")?)
    }
    fn change<T>(&self, f: impl FnOnce(&Session) -> Result<T>) -> Result<T> {
        let mut guard = self
            .project
            .lock()
            .map_err(|_| anyhow::anyhow!("project lock poisoned"))?;
        let session = guard
            .as_mut()
            .context("プロジェクトを作成または開いてください")?;
        let value = f(session)?;
        session.changed();
        drop(guard);
        self.saver.changed()?;
        Ok(value)
    }
    fn snapshot(&self) -> Result<()> {
        let snapshot = self.read(|s| {
            Ok(Snapshot {
                character_previews: s
                    .library
                    .characters()?
                    .into_iter()
                    .filter_map(|c| match s.library.character_preview_path(&c.id) {
                        Ok(Some(_)) => Some(Ok(c.id)),
                        Ok(None) => None,
                        Err(e) => Some(Err(e)),
                    })
                    .collect::<Result<_, _>>()?,
                editor: s.library.editor()?,
                characters: s.library.characters()?,
                takes: s.library.takes()?,
                voices: s.library.voices()?,
            })
        })?;
        self.events
            .send(Event::Status(project::status(&self.project)?))?;
        self.events.send(Event::Snapshot(snapshot))?;
        Ok(())
    }
    fn generate_with(
        &self,
        requests: Vec<TakeRequest>,
        mut infer: impl FnMut(
            irodoru_engine::TTSJob,
        ) -> Result<(
            irodoru_engine::GenerationConditions,
            irodoru_engine::SynthOutput,
        )>,
    ) -> Result<()> {
        let prepared = self.read(|s| generation::prepare(&s.library, requests.clone()))?;
        for (request, prepared) in requests.into_iter().zip(prepared) {
            if self.stop.load(Ordering::SeqCst) {
                break;
            }
            self.events
                .send(Event::RowStarted(request.row_id.clone()))?;
            let result = generation::synthesize(prepared, &mut infer)
                .and_then(|(take, wav)| self.change(|s| Ok(s.library.save_take(&take, &wav)?)));
            if let Err(e) = result {
                self.events
                    .send(Event::RowFailed(request.row_id, format!("{e:#}")))?;
            }
        }
        self.snapshot()?;
        Ok(())
    }
    fn execute(&mut self, command: Command) -> Result<()> {
        if !matches!(command, Command::SaveEditor(_))
            && let Some(error) = &self.editor_error
        {
            anyhow::bail!("台本を保存できませんでした。保存を再試行してください: {error}");
        }
        match command {
            Command::Open { path, create } => {
                self.saver.flush()?;
                let session = self.window_project.open(&path, create)?;
                *self
                    .project
                    .lock()
                    .map_err(|_| anyhow::anyhow!("project lock poisoned"))? = Some(session);
                if create {
                    self.saver.changed()?;
                }
                self.snapshot()?;
            }
            Command::SaveEditor(editor) => {
                let result = self.change(|s| Ok(s.library.save_editor(&editor)?));
                self.editor_error = result.as_ref().err().map(|e| format!("{e:#}"));
                result?;
            }
            Command::Flush { close } => {
                self.saver.flush()?;
                if close {
                    self.events.send(Event::Close)?;
                }
            }
            Command::SaveCharacter { id, input, preview } => {
                self.change(|s| {
                    let c = match id {
                        Some(id) => s.library.update_character(id, input)?,
                        None => s.library.create_character(input)?,
                    };
                    if let Some(preview) = preview {
                        s.library
                            .save_character_preview(&c.id, &s.previews, &preview)?;
                    }
                    Ok(())
                })?;
                self.snapshot()?;
            }
            Command::DeleteCharacter(id) => {
                self.change(|s| Ok(s.library.delete_character(&id)?))?;
                self.snapshot()?;
            }
            Command::DeleteTake(id) => {
                self.change(|s| Ok(s.library.delete_take(&id)?))?;
                self.snapshot()?;
            }
            Command::ImportVoice(path) => {
                let wav = crate::audio::import_audio(&path)?;
                let id = format!("reference-{}", new_id());
                self.change(|s| Ok(s.library.save_voice(&id, &wav)?))?;
                self.events.send(Event::Voice(id))?;
                self.snapshot()?;
            }
            Command::ImportIcon(path) => {
                self.events
                    .send(Event::Icon(crate::audio::import_icon(&path)?))?;
            }
            Command::Generate(requests) => {
                let inference = self.inference.clone();
                self.generate_with(requests, |job| inference.synthesize(job))?;
            }
            Command::Preview {
                input,
                description,
                voice,
            } => {
                let prepared = self
                    .read(|s| generation::prepare_preview(&s.library, input, description, voice))?;
                let (take, wav) =
                    generation::synthesize(prepared, |job| self.inference.synthesize(job))?;
                self.read(|s| Ok(s.previews.save_take(&take, &wav)?))?;
                self.events.send(Event::Preview(take.id))?;
            }
            Command::RemovePreview(id) => {
                self.read(|s| Ok(s.previews.delete_take(&id)?))?;
            }
            Command::Audio { id, preview } => {
                let wav = self.read(|s| {
                    Ok(if preview {
                        s.previews.audio(&id)?
                    } else {
                        s.library.audio(&id)?
                    })
                })?;
                self.events.send(Event::Audio(wav))?;
            }
            Command::ExportCharacter { id, path } => {
                let source = self
                    .read(|s| Ok(s.library.character_preview_path(&id)?))?
                    .context("サンプル音声がありません")?;
                crate::files::atomic_bytes(&path, &std::fs::read(source)?)?;
            }
            Command::CharacterAudio(id) => {
                let path = self
                    .read(|s| Ok(s.library.character_preview_path(&id)?))?
                    .context("サンプル音声がありません")?;
                self.events.send(Event::Audio(std::fs::read(path)?))?;
            }
            Command::Export { id, preview, path } => {
                let wav = self.read(|s| {
                    Ok(if preview {
                        s.previews.audio(&id)?
                    } else {
                        s.library.audio(&id)?
                    })
                })?;
                crate::files::atomic_bytes(&path, &wav)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::EditorState;
    use std::sync::Mutex;
    fn worker() -> (Worker, mpsc::Receiver<Event>) {
        let project = Arc::new(Mutex::new(None));
        let saver = Saver::start(project.clone(), |_| {}).unwrap();
        let (events, rx) = mpsc::channel();
        (
            Worker {
                project,
                saver,
                events,
                inference: Arc::new(
                    Inference::start_with(|| anyhow::bail!("test: no models")).unwrap(),
                ),
                stop: Arc::new(AtomicBool::new(false)),
                editor_error: None,
                window_project: Default::default(),
            },
            rx,
        )
    }
    #[test]
    fn failed_editor_write_blocks_close_and_switch_until_retry() {
        let dir = tempfile::tempdir().unwrap();
        let (mut w, rx) = worker();
        w.execute(Command::Open {
            path: dir.path().join("old.zip"),
            create: true,
        })
        .unwrap();
        w.saver.flush().unwrap();
        let yaml = w
            .read(|s| Ok(s.library.root().join("project.yaml")))
            .unwrap();
        let original = std::fs::read(&yaml).unwrap();
        std::fs::remove_file(&yaml).unwrap();
        std::fs::create_dir(&yaml).unwrap();
        assert!(
            w.execute(Command::SaveEditor(EditorState::default()))
                .is_err()
        );
        assert!(w.execute(Command::Flush { close: true }).is_err());
        assert!(
            w.execute(Command::Open {
                path: dir.path().join("new.zip"),
                create: true
            })
            .is_err()
        );
        assert!(!rx.try_iter().any(|event| matches!(event, Event::Close)));
        assert!(!dir.path().join("new.zip").exists());
        std::fs::remove_dir(&yaml).unwrap();
        std::fs::write(&yaml, original).unwrap();
        w.execute(Command::SaveEditor(EditorState::default()))
            .unwrap();
        w.execute(Command::Flush { close: true }).unwrap();
        assert!(rx.try_iter().any(|event| matches!(event, Event::Close)));
    }
    #[test]
    fn corrupt_project_does_not_replace_active_session() {
        let dir = tempfile::tempdir().unwrap();
        let (mut w, _rx) = worker();
        let old = dir.path().join("old.zip");
        w.execute(Command::Open {
            path: old.clone(),
            create: true,
        })
        .unwrap();
        let invalid = dir.path().join("bad.zip");
        std::fs::write(&invalid, b"not a zip").unwrap();
        assert!(
            w.execute(Command::Open {
                path: invalid,
                create: false
            })
            .is_err()
        );
        assert_eq!(
            w.read(|s| Ok(s.path.clone())).unwrap(),
            old.canonicalize().unwrap()
        );
    }
}

#[cfg(test)]
mod queue_tests {
    use super::*;
    use crate::domain::{CharacterInput, EditorState};
    use irodoru_engine::{Audio, GenerationConditions, SynthOutput};
    use std::sync::Mutex;
    #[test]
    fn stop_finishes_current_row_and_failed_regeneration_keeps_audio() {
        let dir = tempfile::tempdir().unwrap();
        let project = Arc::new(Mutex::new(Some(
            Session::create(dir.path().join("project.zip")).unwrap(),
        )));
        let saver = Saver::start(project.clone(), |_| {}).unwrap();
        let (events, _rx) = mpsc::channel();
        let w = Worker {
            project,
            saver,
            events,
            inference: Arc::new(Inference::start_with(|| anyhow::bail!("unused")).unwrap()),
            stop: Arc::new(AtomicBool::new(false)),
            editor_error: None,
            window_project: Default::default(),
        };
        let character = w
            .change(|s| {
                Ok(s.library.create_character(CharacterInput {
                    name: "声".into(),
                    description: "明るい声".into(),
                    voice: None,
                    color: None,
                    icon: None,
                    sample_text: None,
                })?)
            })
            .unwrap();
        let rows = (0..2)
            .map(|_| {
                let mut r =
                    crate::workspace::editor::new_row(character.id.clone(), Default::default());
                r.text = "こんにちは".into();
                r
            })
            .collect::<Vec<_>>();
        w.change(|s| {
            Ok(s.library.save_editor(&EditorState {
                rows: rows.clone(),
                ..Default::default()
            })?)
        })
        .unwrap();
        let requests =
            crate::workspace::editor::requests(&rows, &[character], &Default::default()).unwrap();
        let mut calls = 0;
        w.generate_with(requests.clone(), |job| {
            calls += 1;
            w.stop.store(true, Ordering::SeqCst);
            Ok((
                GenerationConditions::from_params("anime".into(), &job.params, 42, 1, 48000),
                SynthOutput {
                    audio: Audio::new(vec![0.1; 48000], 48000),
                    seed: 42,
                    num_steps: 1,
                    timings: vec![],
                },
            ))
        })
        .unwrap();
        assert_eq!(calls, 1);
        let takes = w.read(|s| Ok(s.library.takes()?)).unwrap();
        assert_eq!(takes.len(), 1);
        let wav = w.read(|s| Ok(s.library.audio(&takes[0].id)?)).unwrap();
        w.stop.store(false, Ordering::SeqCst);
        w.generate_with(requests, |_| anyhow::bail!("failed inference"))
            .unwrap();
        let after = w.read(|s| Ok(s.library.takes()?)).unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].id, takes[0].id);
        assert_eq!(w.read(|s| Ok(s.library.audio(&after[0].id)?)).unwrap(), wav);
    }
}
