use super::archive::{snapshot, write_archive};
use crate::domain::{AudioSource, new_id};
use std::io::Write;
use std::{fs::File, path::PathBuf};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

use super::*;
use crate::{domain::CharaInput, domain::Line};
fn assert_same_audio(actual: &[u8], expected: &[u8]) {
    let actual = irodoru_engine::Audio::from_wav(actual).unwrap();
    let expected = irodoru_engine::Audio::from_wav(expected).unwrap();
    assert_eq!(actual.sample_rate, expected.sample_rate);
    assert!(
        actual.samples == expected.samples,
        "PCM changed while updating metadata"
    );
}
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
    let voice = session
        .library
        .save_voice("source", &wav.into_inner())
        .unwrap();
    let chara = session
        .library
        .create_chara(CharaInput {
            name: "さくら".into(),
            description: "落ち着いた声".into(),
            voice_ids: vec![voice],
            color: None,
            sample_text: None,
            icon: None,
        })
        .unwrap();
    let mut editor = session.library.editor().unwrap();
    editor.active_chara = chara.id.clone();
    editor.lines.push(Line {
        takes: Vec::new(),
        id: "line".into(),
        chara_id: chara.id,
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
        assert_eq!(reopened.library.editor().unwrap().lines[0].text, "最初");
        let chara = reopened.library.charas().unwrap().remove(0);
        assert!(
            !reopened
                .library
                .audio(&chara.voice_ids[0])
                .unwrap()
                .is_empty()
        );
        let zip = ZipArchive::new(File::open(moved).unwrap()).unwrap();
        let names: Vec<_> = zip
            .file_names()
            .collect::<zip::result::ZipResult<_>>()
            .unwrap();
        assert!(names.iter().any(|name| name == "project.yaml"));
        assert!(names.iter().all(|name| !name.starts_with("lines/")));
        let document: serde_yaml::Value = serde_yaml::from_reader(
            ZipArchive::new(File::open(&reopened.path).unwrap())
                .unwrap()
                .by_name("project.yaml")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(document["version"].as_u64(), Some(0));
        assert_eq!(document.as_mapping().unwrap().len(), 2);
        assert_eq!(document["lines"]["line"]["text"].as_str(), Some("最初"));
        assert!(names.iter().any(|name| name.ends_with("/chara.yaml")));
        assert!(names.iter().all(|name| !name.starts_with('.')));
        assert!(names.iter().all(|name| !name.ends_with("meta.yaml")));
        assert!(
            names
                .iter()
                .filter(|name| name.starts_with("voices/"))
                .all(|name| name.ends_with(".wav") && name.split('/').count() == 2)
        );
    }
}
#[test]
fn take_history_selection_and_chara_preview_survive_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let session = fixture(dir.path().join("project.iro"));
    let chara = session.library.charas().unwrap().remove(0);
    let wav = session.library.audio(&chara.voice_ids[0]).unwrap();
    let take = crate::domain::Take {
        conditions: irodoru_engine::GenerationConditions {
            text: "最初".into(),
            model: "anime".into(),
            seed: 42,
            num_steps: 1,
            ..crate::domain::Take::test_conditions()
        },
        id: new_id(),
        line_id: "line".into(),
        chara_id: Some(chara.id.clone()),
        chara_name: chara.name,
        additional_prompt: String::new(),
        voice_ids: chara.voice_ids,
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
        .save_chara_preview(&chara.id, &session.previews, &take.id)
        .unwrap();
    for source in [
        AudioSource::Take(latest.id.clone()),
        AudioSource::Preview(take.id.clone()),
        AudioSource::CharaPreview(chara.id.clone()),
    ] {
        assert_same_audio(&session.audio(&source).unwrap(), &wav);
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
        reopened.library.editor().unwrap().lines[0].takes,
        vec![latest.id.clone(), take.id.clone()]
    );
    assert_eq!(
        reopened.library.editor().unwrap().lines[0]
            .take_id
            .as_deref(),
        Some(latest.id.as_str())
    );
    assert_same_audio(&reopened.library.audio(&take.id).unwrap(), &wav);
    assert_same_audio(&reopened.library.audio(&latest.id).unwrap(), &wav);
    assert_same_audio(
        &std::fs::read(
            reopened
                .library
                .chara_preview_path(&chara.id)
                .unwrap()
                .unwrap(),
        )
        .unwrap(),
        &wav,
    );
    reopened
        .library
        .select_take(&take.line_id, Some(&take.id))
        .unwrap();
    reopened.library.save_editor(&stale_editor).unwrap();
    assert_eq!(
        reopened.library.editor().unwrap().lines[0]
            .take_id
            .as_deref(),
        Some(take.id.as_str())
    );
    write_archive(&snapshot(&reopened).unwrap()).unwrap();
    let reopened = Session::open(reopened.path.clone()).unwrap();
    assert_eq!(
        reopened.library.editor().unwrap().lines[0]
            .take_id
            .as_deref(),
        Some(take.id.as_str())
    );
    assert!(
        reopened
            .library
            .select_take("another-line", Some(&take.id))
            .is_err()
    );
    reopened.library.select_take(&take.line_id, None).unwrap();
    reopened.library.save_editor(&stale_editor).unwrap();
    assert!(
        reopened.library.editor().unwrap().lines[0]
            .take_id
            .is_none()
    );
    reopened
        .library
        .select_take(&take.line_id, Some(&latest.id))
        .unwrap();
    reopened.library.delete_take(&latest.id).unwrap();
    reopened.library.save_editor(&stale_editor).unwrap();
    let line = &reopened.library.editor().unwrap().lines[0];
    assert_eq!(line.takes, vec![take.id.clone()]);
    assert!(line.take_id.is_none());
    reopened.library.save_editor(&Default::default()).unwrap();
    assert!(reopened.library.takes().unwrap().is_empty());
    assert!(reopened.library.audio(&take.id).is_ok());
}
#[test]
fn snapshot_keeps_the_content_captured_before_editing() {
    let dir = tempfile::tempdir().unwrap();
    let session = fixture(dir.path().join("project.iro"));
    let old = snapshot(&session).unwrap();
    let mut editor = session.library.editor().unwrap();
    editor.lines[0].text = "更新".into();
    session.library.save_editor(&editor).unwrap();

    write_archive(&old).unwrap();
    let reopened = Session::open(session.path.clone()).unwrap();
    assert_eq!(reopened.library.editor().unwrap().lines[0].text, "最初");
    assert_eq!(session.library.editor().unwrap().lines[0].text, "更新");
}
#[test]
fn invalid_archives_do_not_escape_the_project_or_silently_open_empty() {
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "../outside",
        "/outside",
        "charas/../../outside",
        "charas\\outside",
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
fn line_export_uses_selected_takes_in_order_without_overwriting() {
    let root = tempfile::tempdir().unwrap();
    let session = fixture(root.path().join("lines.iro"));
    let chara = session.library.charas().unwrap().remove(0);
    let wav = session.library.audio(&chara.voice_ids[0]).unwrap();
    let second_wav = irodoru_engine::Audio::new(vec![0.25; 24], 24000)
        .to_wav()
        .unwrap();
    let mut editor = session.library.editor().unwrap();
    let mut second_line = editor.lines[0].clone();
    second_line.id = "second".into();
    editor.lines.push(second_line);
    session.library.save_editor(&editor).unwrap();
    let first = crate::domain::Take {
        conditions: irodoru_engine::GenerationConditions {
            text: "音声".into(),
            model: "anime".into(),
            seed: 42,
            num_steps: 1,
            ..crate::domain::Take::test_conditions()
        },
        id: new_id(),
        line_id: "line".into(),
        chara_id: Some(chara.id),
        chara_name: chara.name,
        additional_prompt: String::new(),
        voice_ids: chara.voice_ids,
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
        line_id: "second".into(),
        ..first.clone()
    };
    session.library.save_take(&second, &second_wav).unwrap();
    let wav = session.library.audio(&first.id).unwrap();
    let second_wav = session.library.audio(&second.id).unwrap();
    let mut editor = session.library.editor().unwrap();
    editor.lines.reverse();
    session.library.save_editor(&editor).unwrap();

    let output = tempfile::tempdir().unwrap();
    let (directory, count) = session
        .export_lines(output.path(), ExportNaming::Number)
        .unwrap();
    assert_eq!(count, 2);
    assert_eq!(
        std::fs::read(directory.join("001.wav")).unwrap(),
        second_wav
    );
    assert_eq!(std::fs::read(directory.join("002.wav")).unwrap(), wav);
    let (another, _) = session
        .export_lines(output.path(), ExportNaming::Number)
        .unwrap();
    assert_ne!(directory, another);
    assert_eq!(std::fs::read(directory.join("002.wav")).unwrap(), wav);
    for (naming, filename) in [
        (ExportNaming::Chara, "001_さくら.wav"),
        (ExportNaming::Dialogue, "001_音声.wav"),
        (ExportNaming::CharaDialogue, "001_さくら_音声.wav"),
    ] {
        let (named, _) = session.export_lines(output.path(), naming).unwrap();
        assert_eq!(std::fs::read(named.join(filename)).unwrap(), second_wav);
    }
    let unusual = crate::domain::Take {
        id: new_id(),
        chara_name: "../CON:声\\名前".into(),
        conditions: irodoru_engine::GenerationConditions {
            text: "長いセリフ/\n".repeat(40),
            ..second.conditions.clone()
        },
        ..second.clone()
    };
    session.library.save_take(&unusual, &second_wav).unwrap();
    let (safe, _) = session
        .export_lines(output.path(), ExportNaming::CharaDialogue)
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
            .export_lines(output.path(), ExportNaming::Number)
            .unwrap_err()
            .to_string()
            .contains("2行目")
    );
    assert_eq!(std::fs::read_dir(output.path()).unwrap().count(), 6);
}

fn input_from_chara(c: crate::domain::Chara) -> CharaInput {
    CharaInput {
        name: c.name,
        description: c.description,
        voice_ids: c.voice_ids,
        color: Some(c.color),
        sample_text: Some(c.sample_text),
        icon: c.icon,
    }
}

#[test]
fn shared_generated_references_survive_line_deletion_and_chara_transfers() {
    let dir = tempfile::tempdir().unwrap();
    let session = fixture(dir.path().join("source.iro"));
    let original = session.library.charas().unwrap().remove(0);
    let wav = session.library.audio(&original.voice_ids[0]).unwrap();
    let take = crate::domain::Take {
        id: new_id(),
        line_id: "line".into(),
        chara_id: Some(original.id.clone()),
        chara_name: original.name.clone(),
        voice_ids: original.voice_ids.clone(),
        additional_prompt: String::new(),
        conditions: irodoru_engine::GenerationConditions {
            text: "再利用する音声".into(),
            model: "anime".into(),
            seed: 123,
            num_steps: 30,
            ..crate::domain::Take::test_conditions()
        },
        elapsed_ms: 12,
        audio_seconds: 0.001,
        created_at: 1,
    };
    session.library.save_take(&take, &wav).unwrap();
    let mut input = input_from_chara(original.clone());
    input.voice_ids.push(take.id.clone());
    session
        .library
        .update_chara(original.id.clone(), input.clone())
        .unwrap();
    let second = session.library.create_chara(input.clone()).unwrap();
    session.library.save_editor(&Default::default()).unwrap();
    session.library.delete_take(&take.id).unwrap();
    assert!(session.library.takes().unwrap().is_empty());
    assert_same_audio(&session.library.audio(&take.id).unwrap(), &wav);
    write_archive(&snapshot(&session).unwrap()).unwrap();
    let reopened = Session::open(session.path.clone()).unwrap();
    assert!(reopened.library.editor().unwrap().lines.is_empty());
    assert_eq!(
        reopened.library.chara(&original.id).unwrap().voice_ids,
        input.voice_ids
    );
    assert_eq!(
        reopened.library.chara(&second.id).unwrap().voice_ids,
        input.voice_ids
    );
    assert_eq!(reopened.library.voices().unwrap().len(), 2);

    let bundle = dir.path().join("chara.irochar");
    reopened.export_chara(input.clone(), &bundle).unwrap();
    let zip = ZipArchive::new(File::open(&bundle).unwrap()).unwrap();
    let names: Vec<_> = zip
        .file_names()
        .collect::<zip::result::ZipResult<_>>()
        .unwrap();
    assert!(names.iter().all(|name| !name.starts_with("voices/")));
    assert_eq!(
        names
            .iter()
            .filter(|name| name.contains("/voices/"))
            .count(),
        2
    );
    // The bundle remains usable after deleting the source project and working files.
    drop(reopened);
    drop(session);
    std::fs::remove_file(dir.path().join("source.iro")).unwrap();
    let target = Session::create(dir.path().join("target.iro")).unwrap();
    target.import_chara(&bundle).unwrap();
    target.import_chara(&bundle).unwrap();
    let charas = target.library.charas().unwrap();
    assert_eq!(charas.len(), 2);
    assert_ne!(charas[0].id, charas[1].id);
    assert_ne!(charas[0].voice_ids, charas[1].voice_ids);
    for c in charas {
        assert_eq!(c.name, original.name);
        assert_eq!(c.voice_ids.len(), 2);
        for id in c.voice_ids {
            assert_same_audio(&target.library.audio(&id).unwrap(), &wav);
        }
    }
    write_archive(&snapshot(&target).unwrap()).unwrap();
    Session::open(target.path.clone()).unwrap();
}

#[test]
fn chara_only_project_copies_owned_references_and_preserves_source_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    let source = fixture(dir.path().join("source.iro"));
    let c = source.library.charas().unwrap().remove(0);
    source.library.create_chara(input_from_chara(c)).unwrap();
    write_archive(&snapshot(&source).unwrap()).unwrap();
    let original = std::fs::read(&source.path).unwrap();
    let mut project = Project::new(|_| {}).unwrap();
    project.open(&source.path, false).unwrap();
    assert!(project.create_from_charas(&source.path).is_err());
    assert_eq!(std::fs::read(&source.path).unwrap(), original);
    assert_eq!(
        project
            .read(|s| Ok(s.library.editor()?.lines.len()))
            .unwrap(),
        1
    );
    let target = dir.path().join("charas.iro");
    project.create_from_charas(&target).unwrap();
    project.flush().unwrap();
    let target = Session::open(target).unwrap();
    assert!(target.library.editor().unwrap().lines.is_empty());
    assert_eq!(target.library.charas().unwrap().len(), 2);
    assert_eq!(target.library.voices().unwrap().len(), 1);
    assert_eq!(
        std::fs::read_dir(target.library.root().join("voices"))
            .unwrap()
            .count(),
        0
    );
    for c in target.library.charas().unwrap() {
        assert!(
            target
                .library
                .root()
                .join("charas")
                .join(&c.id)
                .join("voices")
                .join(format!("{}.wav", c.voice_ids[0]))
                .is_file()
        );
    }
    assert_eq!(std::fs::read(&source.path).unwrap(), original);
}

#[test]
fn failed_chara_import_leaves_no_partial_voice_records() {
    let dir = tempfile::tempdir().unwrap();
    let source = fixture(dir.path().join("source.iro"));
    let input: CharaInput = source.library.charas().unwrap().remove(0).into();
    let bundle = dir.path().join("chara.irochar");
    source.export_chara(input, &bundle).unwrap();
    let target = Session::create(dir.path().join("target.iro")).unwrap();
    let charas = target.library.root().join("charas");
    std::fs::remove_dir(&charas).unwrap();
    std::fs::write(&charas, b"force install failure").unwrap();
    assert!(target.import_chara(&bundle).is_err());
    assert_eq!(
        std::fs::read_dir(target.library.root().join("voices"))
            .unwrap()
            .count(),
        0
    );
    std::fs::remove_file(&charas).unwrap();
    std::fs::create_dir(&charas).unwrap();
    assert!(target.library.voices().unwrap().is_empty());
    assert!(target.library.charas().unwrap().is_empty());
    assert!(target.library.editor().unwrap().lines.is_empty());
}

#[test]
fn unsupported_versions_and_sidecar_layouts_are_rejected_without_modification() {
    let dir = tempfile::tempdir().unwrap();
    let source = fixture(dir.path().join("source.iro"));
    let document = std::fs::read_to_string(source.library.root().join("project.yaml")).unwrap();
    for (document, sidecar) in [
        (document.replace("version: 0", "version: 1"), None),
        (document, Some(format!("voices/{}/meta.yaml", new_id()))),
    ] {
        let path = dir.path().join("unsupported.iro");
        let mut zip = ZipWriter::new(File::create(&path).unwrap());
        zip.start_file("project.yaml", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(document.as_bytes()).unwrap();
        if let Some(sidecar) = sidecar {
            zip.start_file(sidecar, SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"{}").unwrap();
        }
        zip.finish().unwrap();
        let original = std::fs::read(&path).unwrap();
        assert!(Session::open(path.clone()).is_err());
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
}

#[test]
fn missing_chara_reference_is_rejected_even_when_source_audio_exists() {
    let dir = tempfile::tempdir().unwrap();
    let session = fixture(dir.path().join("missing-reference.iro"));
    let c = session.library.charas().unwrap().remove(0);
    assert!(session.library.audio(&c.voice_ids[0]).is_ok());
    std::fs::remove_file(
        session
            .library
            .root()
            .join("charas")
            .join(&c.id)
            .join("voices")
            .join(format!("{}.wav", c.voice_ids[0])),
    )
    .unwrap();
    write_archive(&snapshot(&session).unwrap()).unwrap();
    assert!(Session::open(session.path.clone()).is_err());
}
