use super::archive::{snapshot, write_archive};
use crate::domain::new_id;
use std::io::{Read, Write};
use std::{
    fs::File,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

use super::*;
use crate::{domain::CharacterInput, domain::ScriptRow};
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
    let take = crate::domain::Take {
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
    let latest = crate::domain::Take {
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
