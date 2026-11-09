use super::archive::{snapshot, write_archive};
use crate::domain::{AudioSource, new_id};
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
    for extension in ["iro", "IRO"] {
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
fn take_history_selection_and_character_preview_survive_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let session = fixture(dir.path().join("project.iro"));
    let character = session.library.characters().unwrap().remove(0);
    let wav = session
        .library
        .voice(character.voice.as_ref().unwrap())
        .unwrap();
    let take = crate::domain::Take {
        filename: None,
        conditions: None,
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
    for source in [
        AudioSource::Take(latest.id.clone()),
        AudioSource::Preview(take.id.clone()),
        AudioSource::CharacterPreview(character.id.clone()),
    ] {
        assert_eq!(session.audio(&source).unwrap(), wav);
    }
    write_archive(&snapshot(&session).unwrap()).unwrap();
    let reopened = Session::open(session.path.clone()).unwrap();
    assert!(
        reopened
            .audio(&AudioSource::Preview(take.id.clone()))
            .is_err()
    );
    assert_eq!(reopened.library.takes().unwrap().len(), 2);
    assert_eq!(
        reopened.library.editor().unwrap().rows[0]
            .take_id
            .as_deref(),
        Some(latest.id.as_str())
    );
    assert_eq!(reopened.library.audio(&take.id).unwrap(), wav);
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
    reopened
        .library
        .select_take(&take.row_id, Some(&take.id))
        .unwrap();
    reopened.library.save_editor(&stale_editor).unwrap();
    assert_eq!(
        reopened.library.editor().unwrap().rows[0]
            .take_id
            .as_deref(),
        Some(take.id.as_str())
    );
    write_archive(&snapshot(&reopened).unwrap()).unwrap();
    let reopened = Session::open(reopened.path.clone()).unwrap();
    assert_eq!(
        reopened.library.editor().unwrap().rows[0]
            .take_id
            .as_deref(),
        Some(take.id.as_str())
    );
    assert!(
        reopened
            .library
            .select_take("another-row", Some(&take.id))
            .is_err()
    );
    reopened.library.select_take(&take.row_id, None).unwrap();
    reopened.library.save_editor(&stale_editor).unwrap();
    assert!(reopened.library.editor().unwrap().rows[0].take_id.is_none());
    reopened.library.save_editor(&Default::default()).unwrap();
    assert!(reopened.library.takes().unwrap().is_empty());
}
#[test]
fn snapshot_keeps_the_content_captured_before_editing() {
    let dir = tempfile::tempdir().unwrap();
    let session = fixture(dir.path().join("project.iro"));
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
        let path = dir.path().join("bad.iro");
        let mut zip = ZipWriter::new(File::create(&path).unwrap());
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(b"bad").unwrap();
        zip.finish().unwrap();
        assert!(Session::open(path).is_err(), "{name}");
    }
    let path = dir.path().join("empty.iro");
    ZipWriter::new(File::create(&path).unwrap())
        .finish()
        .unwrap();
    assert!(Session::open(path).is_err());
    assert!(!dir.path().join("outside").exists());
}

#[test]
fn script_export_uses_selected_takes_in_order_without_overwriting() {
    let root = tempfile::tempdir().unwrap();
    let session = fixture(root.path().join("script.iro"));
    let character = session.library.characters().unwrap().remove(0);
    let wav = session
        .library
        .voice(character.voice.as_ref().unwrap())
        .unwrap();
    let mut second_wav = wav.clone();
    *second_wav.last_mut().unwrap() ^= 1;
    let mut editor = session.library.editor().unwrap();
    let mut second_row = editor.rows[0].clone();
    second_row.id = "second".into();
    editor.rows.push(second_row);
    session.library.save_editor(&editor).unwrap();
    let first = crate::domain::Take {
        filename: None,
        conditions: None,
        id: new_id(),
        row_id: "line".into(),
        character_id: Some(character.id),
        character_name: character.name,
        description: character.description,
        additional_prompt: String::new(),
        voice: character.voice,
        text: "音声".into(),
        model: "anime".into(),
        seed: 42,
        num_steps: 1,
        elapsed_ms: 1,
        audio_seconds: 0.001,
        created_at: 1,
    };
    session.library.save_take(&first, &wav).unwrap();
    let newer = crate::domain::Take {
        id: new_id(),
        created_at: 2,
        ..first.clone()
    };
    session.library.save_take(&newer, &second_wav).unwrap();
    session
        .library
        .select_take("line", Some(&first.id))
        .unwrap();
    let second = crate::domain::Take {
        id: new_id(),
        row_id: "second".into(),
        ..first.clone()
    };
    session.library.save_take(&second, &second_wav).unwrap();
    let mut editor = session.library.editor().unwrap();
    editor.rows.reverse();
    session.library.save_editor(&editor).unwrap();

    let output = tempfile::tempdir().unwrap();
    let (directory, count) = session
        .export_script(output.path(), ExportNaming::Number)
        .unwrap();
    assert_eq!(count, 2);
    assert_eq!(
        std::fs::read(directory.join("001.wav")).unwrap(),
        second_wav
    );
    assert_eq!(std::fs::read(directory.join("002.wav")).unwrap(), wav);
    let (another, _) = session
        .export_script(output.path(), ExportNaming::Number)
        .unwrap();
    assert_ne!(directory, another);
    assert_eq!(std::fs::read(directory.join("002.wav")).unwrap(), wav);
    for (naming, filename) in [
        (ExportNaming::Character, "001_さくら.wav"),
        (ExportNaming::Dialogue, "001_音声.wav"),
        (ExportNaming::CharacterDialogue, "001_さくら_音声.wav"),
    ] {
        let (named, _) = session.export_script(output.path(), naming).unwrap();
        assert_eq!(std::fs::read(named.join(filename)).unwrap(), second_wav);
    }
    let unusual = crate::domain::Take {
        id: new_id(),
        character_name: "../CON:声\\名前".into(),
        text: "長いセリフ/\n".repeat(40),
        ..second.clone()
    };
    session.library.save_take(&unusual, &second_wav).unwrap();
    let (safe, _) = session
        .export_script(output.path(), ExportNaming::CharacterDialogue)
        .unwrap();
    let names = std::fs::read_dir(&safe)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(names.len(), 2);
    assert!(
        names
            .iter()
            .all(|name| name.len() < 200 && !name.contains(['/', '\\', ':', '\n']))
    );
    session.library.select_take("line", None).unwrap();
    assert!(
        session
            .export_script(output.path(), ExportNaming::Number)
            .unwrap_err()
            .to_string()
            .contains("2行目")
    );
    assert_eq!(std::fs::read_dir(output.path()).unwrap().count(), 6);
}
