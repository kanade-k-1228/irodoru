use super::archive::{snapshot, write_archive};
use crate::domain::new_id;
use std::io::Write;
use std::{fs::File, path::PathBuf};
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
    for extension in ["iro", "zip"] {
        let dir = tempfile::tempdir().unwrap();
        let session = fixture(dir.path().join(format!("first.{extension}")));
        write_archive(&snapshot(&session).unwrap()).unwrap();
        let moved = dir.path().join(format!("renamed.{extension}"));
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
fn snapshot_keeps_the_content_captured_before_editing() {
    let dir = tempfile::tempdir().unwrap();
    let session = fixture(dir.path().join("project.zip"));
    let old = snapshot(&session).unwrap();
    let mut editor = session.library.editor().unwrap();
    editor.rows[0].text = "更新".into();
    session.library.save_editor(&editor).unwrap();

    write_archive(&old).unwrap();
    let reopened = Session::open(session.path.clone()).unwrap();
    assert_eq!(reopened.library.editor().unwrap().rows[0].text, "最初");
    assert_eq!(session.library.editor().unwrap().rows[0].text, "更新");
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
