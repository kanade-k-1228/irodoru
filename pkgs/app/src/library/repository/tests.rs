use super::*;

fn chara_input(name: &str) -> CharaInput {
    CharaInput {
        name: name.into(),
        description: "落ち着いた声".into(),
        voice_ids: Vec::new(),
        icon: None,
        color: None,
        sample_text: None,
    }
}

#[test]
fn chara_edits_preserve_existing_settings() {
    let dir = tempfile::tempdir().unwrap();
    let library = Library::open(dir.path()).unwrap();
    let mut input = chara_input("さくら");
    input.color = Some("#ABCDEF".into());
    input.sample_text = Some("サンプル文".into());
    input.icon = Some("data:image/png;base64,iVBORw0KGgo=".into());
    let original = library.create_chara(input).unwrap();
    library
        .update_chara(original.id.clone(), chara_input("更新後"))
        .unwrap();

    let reopened = Library::open(dir.path()).unwrap();
    let edited = reopened.chara(&original.id).unwrap();
    assert_eq!(edited.name, "更新後");
    assert_eq!(edited.color, "#abcdef");
    assert_eq!(edited.sample_text, "サンプル文");
    assert_eq!(edited.icon, original.icon);
}

#[test]
fn invalid_names_and_paths_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let library = Library::open(dir.path()).unwrap();
    assert!(library.create_chara(chara_input(" ")).is_err());
    assert!(library.chara("../outside").is_err());
    assert!(library.audio("../outside").is_err());
}

#[test]
fn chara_references_are_owned_copies_and_failed_updates_preserve_them() {
    let dir = tempfile::tempdir().unwrap();
    let library = Library::open(dir.path()).unwrap();
    let wav = irodoru_engine::Audio::new(vec![0.1; 24], 24000)
        .to_wav()
        .unwrap();
    let voice = library.save_voice("reference", &wav).unwrap();
    let mut input = chara_input("さくら");
    input.voice_ids = vec![voice.clone()];
    let first = library.create_chara(input.clone()).unwrap();
    let second = library.create_chara(input.clone()).unwrap();
    let first_path = library.chara_voice_path(&first.id, &voice).unwrap();
    let second_path = library.chara_voice_path(&second.id, &voice).unwrap();
    let original = std::fs::read(&first_path).unwrap();
    assert_eq!(std::fs::read(&second_path).unwrap(), original);
    std::fs::remove_file(library.voice_path(&voice).unwrap()).unwrap();
    assert_eq!(
        library.chara_reference_audio(&first).unwrap(),
        vec![original.clone()]
    );

    input.voice_ids.push(new_id());
    assert!(
        library
            .update_chara(first.id.clone(), input.clone())
            .is_err()
    );
    assert_eq!(
        library.chara(&first.id).unwrap().voice_ids,
        vec![voice.clone()]
    );
    assert_eq!(std::fs::read(&first_path).unwrap(), original);
    input.voice_ids.clear();
    library.update_chara(first.id.clone(), input).unwrap();
    assert!(!first_path.exists());
    assert_eq!(
        library.chara_reference_audio(&second).unwrap(),
        vec![original.clone()]
    );
    library.delete_chara(&first.id).unwrap();
    assert_eq!(std::fs::read(second_path).unwrap(), original);
    assert!(library.takes().unwrap().is_empty());
}

#[test]
fn ordered_line_map_preserves_reordering() {
    let dir = tempfile::tempdir().unwrap();
    let library = Library::open(dir.path()).unwrap();
    let mut editor = crate::domain::EditorState::default();
    for id in ["z", "a"] {
        let mut line = crate::workspace::editor::new_line(String::new(), Default::default());
        line.id = id.into();
        editor.lines.push(line);
    }
    library.save_editor(&editor).unwrap();
    let mut restored = library.editor().unwrap();
    assert_eq!(
        restored
            .lines
            .iter()
            .map(|line| line.id.as_str())
            .collect::<Vec<_>>(),
        ["z", "a"]
    );
    restored.lines.swap(0, 1);
    library.save_editor(&restored).unwrap();
    assert_eq!(library.editor().unwrap().lines[0].id, "a");
}
