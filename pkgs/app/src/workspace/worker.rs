//! Ordered application commands. All storage and synthesis work runs off the UI thread.
use crate::{domain::TakeRequest, project::Project, synthesis::Inference, synthesis::generation};
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
    chara_errors: std::collections::HashMap<String, String>,
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
            chara_errors: Default::default(),
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
            let charas = s.library.charas()?;
            Ok(Snapshot {
                chara_previews: charas
                    .iter()
                    .filter_map(|c| match s.library.chara_preview_path(&c.id) {
                        Ok(Some(_)) => Some(Ok(c.id.clone())),
                        Ok(None) => None,
                        Err(e) => Some(Err(e)),
                    })
                    .collect::<Result<_, _>>()?,
                editor: s.library.editor()?,
                takes: s.library.takes()?,
                voices: s.library.voices()?,
                charas,
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
                    let chara = s.library.chara(&request.chara_id)?;
                    let reference_audio = s.library.chara_reference_audio(&chara)?;
                    Ok(generation::GenerationInput {
                        request,
                        chara,
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
            let line_id = prepared.line_id().to_owned();
            self.events.send(Event::LineStarted(line_id.clone()))?;
            let result = generation::synthesize(prepared, &mut infer).and_then(|(take, wav)| {
                self.project
                    .change(|s| Ok(s.library.save_take(&take, &wav)?))
            });
            if let Err(e) = result {
                self.events
                    .send(Event::LineFailed(line_id, format!("{e:#}")))?;
            }
        }
        self.snapshot()?;
        Ok(())
    }
    fn execute(&mut self, command: Command) -> Result<()> {
        if !matches!(command, Command::SaveEditor(_) | Command::SaveChara { .. })
            && let Some(error) = self.chara_errors.values().next()
        {
            anyhow::bail!("キャラクターを保存できませんでした: {error}");
        }
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
            Command::NewFromCharas(path) => {
                self.project.create_from_charas(&path)?;
                self.snapshot()?;
            }
            Command::ExportChara { input, path } => {
                self.project.read(|s| s.export_chara(input, &path))?;
            }
            Command::ImportChara(path) => {
                self.project.change(|s| s.import_chara(&path))?;
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
            Command::SaveChara { id, input } => {
                if let Some(id) = id {
                    let result = self
                        .project
                        .change(|s| Ok(s.library.update_chara(id.clone(), input)?));
                    match result {
                        Ok(_) => {
                            self.chara_errors.remove(&id);
                        }
                        Err(error) => {
                            self.chara_errors.insert(id, format!("{error:#}"));
                            return Err(error);
                        }
                    }
                } else {
                    let chara = self
                        .project
                        .change(|s| Ok(s.library.create_chara(input)?))?;
                    self.events.send(Event::CharaCreated(chara.id))?;
                    self.snapshot()?;
                }
            }
            Command::DeleteChara(id) => {
                self.project.change(|s| Ok(s.library.delete_chara(&id)?))?;
                self.snapshot()?;
            }
            Command::SelectTake { line_id, take_id } => {
                self.project
                    .change(|s| Ok(s.library.select_take(&line_id, take_id.as_deref())?))?;
                self.snapshot()?;
            }
            Command::DeleteTake(id) => {
                self.project.change(|s| Ok(s.library.delete_take(&id)?))?;
                self.snapshot()?;
            }
            Command::ImportVoices { chara_id, paths } => {
                // Decode every file before changing the project.
                let audio = paths
                    .iter()
                    .map(|path| {
                        Ok((
                            path.file_stem()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned(),
                            crate::audio::import_recording(path)?,
                        ))
                    })
                    .collect::<Result<Vec<_>>>()?;
                let mut chara = self.project.read(|s| Ok(s.library.chara(&chara_id)?))?;
                for (name, wav) in audio {
                    let id = self
                        .project
                        .change(|s| Ok(s.library.save_voice(&name, &wav)?))?;
                    chara.voice_ids.push(id);
                }
                self.project
                    .change(|s| Ok(s.library.update_chara(chara_id, chara.into())?))?;
                self.snapshot()?;
            }
            Command::ImportIcon { chara_id, path } => {
                let icon = crate::icons::import_icon(&path)?;
                self.project.change(|s| {
                    let mut chara = s.library.chara(&chara_id)?;
                    chara.icon = Some(icon);
                    Ok(s.library.update_chara(chara_id, chara.into())?)
                })?;
                self.snapshot()?;
            }
            Command::Generate(requests) => {
                let inference = self.inference.clone();
                self.generate_with(requests, |job| inference.synthesize(job))?;
            }
            Command::Preview {
                chara_id,
                input,
                description,
                voice_ids,
            } => {
                let prepared = self.project.read(|s| {
                    let reference_audio = s.library.reference_audio(&voice_ids)?;
                    generation::prepare_preview(input, description, voice_ids, reference_audio)
                })?;
                let (take, wav) =
                    generation::synthesize(prepared, |job| self.inference.synthesize(job))?;
                self.project.change(|s| {
                    s.previews.save_take(&take, &wav)?;
                    s.library
                        .save_chara_preview(&chara_id, &s.previews, &take.id)?;
                    s.previews.delete_take(&take.id)?;
                    Ok(())
                })?;
                self.snapshot()?;
            }
            Command::Audio(source) => {
                let wav = self.project.read(|s| s.audio(&source))?;
                self.events.send(Event::Audio(wav))?;
            }
            Command::ExportLines { parent, naming } => {
                if let Some(error) = &self.editor_error {
                    anyhow::bail!("台本を保存できないため書き出せません: {error}");
                }
                let (directory, count) = self.project.read(|s| s.export_lines(&parent, naming))?;
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
