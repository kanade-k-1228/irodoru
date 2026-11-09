//! Ordered application commands. All storage and synthesis work runs off the UI thread.
use crate::{
    domain::TakeRequest, domain::new_id, project::Project, synthesis::Inference,
    synthesis::generation,
};
use anyhow::Result;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

use super::protocol::{Command, Event, Snapshot};
pub(super) struct Worker {
    project: Project,
    inference: Arc<Inference>,
    stop: Arc<AtomicBool>,
    events: mpsc::Sender<Event>,
    editor_error: Option<String>,
}
impl Worker {
    pub(super) fn run(
        receiver: mpsc::Receiver<Command>,
        events: mpsc::Sender<Event>,
        inference: Arc<Inference>,
        stop: Arc<AtomicBool>,
    ) {
        let report = events.clone();
        let project = match Project::new(move |s| {
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
            inference,
            stop,
            editor_error: None,
            events,
        };
        while let Ok(command) = receiver.recv() {
            let passive = command.is_passive();
            if let Err(e) = worker.execute(command) {
                let _ = worker.events.send(Event::Error(format!("{e:#}")));
            }
            if !passive {
                let _ = worker.events.send(Event::Done);
            }
        }
    }

    fn snapshot(&self) -> Result<()> {
        let snapshot = self.project.read(|s| {
            let characters = s.library.characters()?;
            Ok(Snapshot {
                character_previews: characters
                    .iter()
                    .filter_map(|c| match s.library.character_preview_path(&c.id) {
                        Ok(Some(_)) => Some(Ok(c.id.clone())),
                        Ok(None) => None,
                        Err(e) => Some(Err(e)),
                    })
                    .collect::<Result<_, _>>()?,
                editor: s.library.editor()?,
                takes: s.library.takes()?,
                voices: s.library.voices(&characters)?,
                characters,
            })
        })?;
        self.events.send(Event::Status(self.project.status()?))?;
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
        let prepared = self.project.read(|s| {
            let inputs = requests
                .into_iter()
                .map(|request| {
                    let character = s.library.character(&request.character_id)?;
                    let reference_audio = character
                        .voice
                        .as_ref()
                        .map(|v| s.library.voice(v))
                        .transpose()?;
                    Ok(generation::GenerationInput {
                        request,
                        character,
                        reference_audio,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            generation::prepare(inputs)
        })?;
        for prepared in prepared {
            if self.stop.load(Ordering::SeqCst) {
                break;
            }
            let row_id = prepared.row_id().to_owned();
            self.events.send(Event::RowStarted(row_id.clone()))?;
            let result = generation::synthesize(prepared, &mut infer).and_then(|(take, wav)| {
                self.project
                    .change(|s| Ok(s.library.save_take(&take, &wav)?))
            });
            if let Err(e) = result {
                self.events
                    .send(Event::RowFailed(row_id, format!("{e:#}")))?;
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
                self.project.open(&path, create)?;
                self.snapshot()?;
            }
            Command::SaveEditor(editor) => {
                let result = self.project.change(|s| Ok(s.library.save_editor(&editor)?));
                self.editor_error = result.as_ref().err().map(|e| format!("{e:#}"));
                result?;
            }
            Command::Flush { close } => {
                self.project.flush()?;
                if close {
                    self.events.send(Event::Close)?;
                }
            }
            Command::SaveCharacter { id, input, preview } => {
                self.project.change(|s| {
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
                self.project
                    .change(|s| Ok(s.library.delete_character(&id)?))?;
                self.snapshot()?;
            }
            Command::SelectTake { row_id, take_id } => {
                self.project
                    .change(|s| Ok(s.library.select_take(&row_id, take_id.as_deref())?))?;
                self.snapshot()?;
            }
            Command::DeleteTake(id) => {
                self.project.change(|s| Ok(s.library.delete_take(&id)?))?;
                self.snapshot()?;
            }
            Command::ImportVoice(path) => {
                let wav = crate::audio::import_audio(&path)?;
                let id = format!("reference-{}", new_id());
                self.project
                    .change(|s| Ok(s.library.save_voice(&id, &wav)?))?;
                self.events.send(Event::Voice(id))?;
                self.snapshot()?;
            }
            Command::ImportIcon(path) => {
                self.events
                    .send(Event::Icon(crate::icons::import_icon(&path)?))?;
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
                let prepared = self.project.read(|s| {
                    let reference_audio = voice.as_ref().map(|v| s.library.voice(v)).transpose()?;
                    generation::prepare_preview(input, description, voice, reference_audio)
                })?;
                let (take, wav) =
                    generation::synthesize(prepared, |job| self.inference.synthesize(job))?;
                self.project
                    .read(|s| Ok(s.previews.save_take(&take, &wav)?))?;
                self.events.send(Event::Preview(take.id))?;
            }
            Command::RemovePreview(id) => {
                self.project.read(|s| Ok(s.previews.delete_take(&id)?))?;
            }
            Command::Audio(source) => {
                let wav = self.project.read(|s| s.audio(&source))?;
                self.events.send(Event::Audio(wav))?;
            }
            Command::ExportScript { parent, naming } => {
                if let Some(error) = &self.editor_error {
                    anyhow::bail!("台本を保存できないため書き出せません: {error}");
                }
                let (directory, count) = self.project.read(|s| s.export_script(&parent, naming))?;
                self.events.send(Event::Exported { directory, count })?;
            }
            Command::Export { source, path } => {
                let wav = self.project.read(|s| s.audio(&source))?;
                crate::files::atomic_bytes(&path, &wav)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CharacterInput, EditorState};
    use irodoru_engine::{Audio, GenerationConditions, SynthOutput};

    fn worker() -> (Worker, mpsc::Receiver<Event>) {
        let project = Project::new(|_| {}).unwrap();
        let (events, rx) = mpsc::channel();
        (
            Worker {
                project,
                events,
                inference: Arc::new(
                    Inference::start_with(|| anyhow::bail!("test: no models")).unwrap(),
                ),
                stop: Arc::new(AtomicBool::new(false)),
                editor_error: None,
            },
            rx,
        )
    }
    #[test]
    fn failed_editor_write_blocks_close_and_switch_until_retry() {
        let dir = tempfile::tempdir().unwrap();
        let (mut w, rx) = worker();
        w.execute(Command::Open {
            path: dir.path().join("old.iro"),
            create: true,
        })
        .unwrap();
        w.project.flush().unwrap();
        let yaml = w
            .project
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
                path: dir.path().join("new.iro"),
                create: true
            })
            .is_err()
        );
        assert!(!rx.try_iter().any(|event| matches!(event, Event::Close)));
        assert!(!dir.path().join("new.iro").exists());
        std::fs::remove_dir(&yaml).unwrap();
        std::fs::write(&yaml, original).unwrap();
        w.execute(Command::SaveEditor(EditorState::default()))
            .unwrap();
        w.execute(Command::Flush { close: true }).unwrap();
        assert!(rx.try_iter().any(|event| matches!(event, Event::Close)));
    }
    #[test]
    fn zip_commands_preserve_the_open_project_and_emit_no_replacement_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let (mut worker, events) = worker();
        worker
            .execute(Command::Open {
                path: dir.path().join("project.IRO"),
                create: true,
            })
            .unwrap();
        worker.project.flush().unwrap();
        let id = worker.project.status().unwrap().id;
        events.try_iter().for_each(drop);
        let zip = dir.path().join("legacy.zip");
        std::fs::copy(dir.path().join("project.IRO"), &zip).unwrap();
        let bytes = std::fs::read(&zip).unwrap();
        for create in [false, true] {
            assert!(
                worker
                    .execute(Command::Open {
                        path: zip.clone(),
                        create
                    })
                    .is_err()
            );
            assert_eq!(worker.project.status().unwrap().id, id);
            assert!(
                !events
                    .try_iter()
                    .any(|event| matches!(event, Event::Snapshot(_)))
            );
            assert_eq!(std::fs::read(&zip).unwrap(), bytes);
        }
    }
    #[test]
    fn stop_finishes_current_row_and_failed_regeneration_keeps_audio() {
        let dir = tempfile::tempdir().unwrap();
        let (mut w, _rx) = worker();
        w.execute(Command::Open {
            path: dir.path().join("project.iro"),
            create: true,
        })
        .unwrap();
        let character = w
            .project
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
        w.project
            .change(|s| {
                Ok(s.library.save_editor(&EditorState {
                    rows: rows.clone(),
                    ..Default::default()
                })?)
            })
            .unwrap();
        let requests =
            crate::workspace::editor::requests(&rows, &[character], &Default::default()).unwrap();
        let mut invalid = requests.clone();
        invalid[1].character_id = "missing".into();
        assert!(
            w.generate_with(invalid, |_| panic!("validate the entire queue first"))
                .is_err()
        );
        assert!(
            w.project
                .read(|s| Ok(s.library.takes()?))
                .unwrap()
                .is_empty()
        );
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
        let takes = w.project.read(|s| Ok(s.library.takes()?)).unwrap();
        assert_eq!(takes.len(), 1);
        let wav = w
            .project
            .read(|s| Ok(s.library.audio(&takes[0].id)?))
            .unwrap();
        w.stop.store(false, Ordering::SeqCst);
        w.generate_with(requests, |_| anyhow::bail!("failed inference"))
            .unwrap();
        let after = w.project.read(|s| Ok(s.library.takes()?)).unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].id, takes[0].id);
        assert_eq!(
            w.project
                .read(|s| Ok(s.library.audio(&after[0].id)?))
                .unwrap(),
            wav
        );
    }
}
