use super::*;

struct TestLibrary(Library);
impl TestLibrary {
    fn new() -> Self {
        Self(
            Library::open(&std::env::temp_dir().join(format!("irodoru-library-{}", new_id())))
                .unwrap(),
        )
    }
}
impl Drop for TestLibrary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0.root);
    }
}
fn character_input(name: &str) -> CharacterInput {
    CharacterInput {
        icon: None,
        color: None,
        sample_text: None,
        name: name.into(),
        description: "落ち着いた声".into(),
        voice: None,
    }
}

#[test]
fn character_icon_persists_and_can_be_removed() {
    let test = TestLibrary::new();
    let mut input = character_input("さくら");
    let icon = "data:image/png;base64,iVBORw0KGgo=";
    input.icon = Some(icon.into());
    let character = test.0.create_character(input).unwrap();
    assert_eq!(
        test.0.character(&character.id).unwrap().icon.as_deref(),
        Some(icon)
    );
    let unchanged = test
        .0
        .update_character(character.id.clone(), character_input("さくら"))
        .unwrap();
    assert_eq!(unchanged.icon.as_deref(), Some(icon));
    let mut removed = character_input("さくら");
    removed.icon = Some(String::new());
    assert!(
        test.0
            .update_character(character.id, removed)
            .unwrap()
            .icon
            .is_none()
    );
    let mut invalid = character_input("不正");
    invalid.icon = Some("data:image/svg+xml;base64,AAAA".into());
    assert!(test.0.create_character(invalid).is_err());
}

#[test]
fn character_presentation_survives_and_missing_take_selection_is_cleared() {
    let test = TestLibrary::new();
    let mut input = character_input("さくら");
    input.color = Some("#ABCDEF".into());
    input.sample_text = Some("サンプル文".into());
    let character = test.0.create_character(input).unwrap();
    let edited = test
        .0
        .update_character(character.id.clone(), character_input("更新後"))
        .unwrap();
    assert_eq!(edited.color, "#abcdef");
    assert_eq!(edited.sample_text, "サンプル文");
    let mut invalid = character_input("不正");
    invalid.color = Some("red".into());
    assert!(
        test.0
            .update_character(character.id.clone(), invalid)
            .is_err()
    );
    let mut editor = crate::domain::EditorState::default();
    editor.rows.push(crate::domain::ScriptRow {
        id: "row".into(),
        text: "本文".into(),
        character_id: character.id.clone(),
        prompt: None,
        settings: None,
        take_id: Some("chosen-take".into()),
    });
    test.0.save_editor(&editor).unwrap();
    let reopened = Library::open(&test.0.root).unwrap();
    assert_eq!(reopened.character(&character.id).unwrap().name, "更新後");
    assert_eq!(reopened.editor().unwrap().rows[0].take_id.as_deref(), None);
    assert!(!valid_wav_filename("../../outside.wav"));
    assert!(valid_wav_filename(&format!("{}.wav", "a".repeat(64))));
}

#[test]
fn regeneration_replaces_previous_audio_and_survives_character_edits() {
    let test = TestLibrary::new();
    let library = &test.0;
    let character = library.create_character(character_input("さくら")).unwrap();
    let first = Take {
        filename: None,
        id: new_id(),
        row_id: "same-row".into(),
        character_id: Some(character.id.clone()),
        character_name: character.name.clone(),
        description: character.description.clone(),
        additional_prompt: String::new(),
        voice: None,
        text: "こんにちは".into(),
        model: "mf".into(),
        seed: 42,
        num_steps: 4,
        elapsed_ms: 100,
        audio_seconds: 1.0,
        created_at: 1,
    };
    library.save_take(&first, b"first wav").unwrap();
    let mut old_metadata = serde_json::to_value(&first).unwrap();
    old_metadata
        .as_object_mut()
        .unwrap()
        .remove("additional_prompt");
    let old_take: Take = serde_json::from_value(old_metadata).unwrap();
    assert!(old_take.additional_prompt.is_empty());
    let second = Take {
        filename: None,
        id: new_id(),
        created_at: 2,
        ..first.clone()
    };
    library.save_take(&second, b"second wav").unwrap();
    assert!(library.save_take(&second, b"replacement").is_err());
    library
        .update_character(character.id.clone(), character_input("編集後"))
        .unwrap();
    let reopened = Library::open(&library.root).unwrap();
    assert_eq!(reopened.characters().unwrap()[0].name, "編集後");
    let takes = reopened.takes().unwrap();
    assert_eq!(takes.len(), 1);
    assert_eq!(takes[0].id, second.id);
    assert_eq!(takes[0].character_name, "さくら");
    assert!(reopened.audio(&first.id).is_err());
    assert_eq!(reopened.audio(&second.id).unwrap(), b"second wav");
    reopened.delete_character(&character.id).unwrap();
    assert_eq!(reopened.takes().unwrap().len(), 1);
    assert!(matches!(
        reopened.update_character(character.id.clone(), character_input("missing")),
        Err(AppError::NotFound(_))
    ));
    reopened.delete_take(&second.id).unwrap();
    assert!(matches!(
        reopened.audio(&first.id),
        Err(AppError::NotFound(_))
    ));
    assert!(reopened.takes().unwrap().is_empty());
}

#[test]
fn incomplete_writes_are_hidden_and_invalid_ids_and_names_are_rejected() {
    let test = TestLibrary::new();
    let library = &test.0;
    std::fs::create_dir(library.root.join("voices").join(format!(".{}", new_id()))).unwrap();
    std::fs::write(
        library
            .root
            .join("characters")
            .join(format!(".{}.tmp", new_id())),
        b"partial",
    )
    .unwrap();
    assert!(library.takes().unwrap().is_empty());
    assert!(library.characters().unwrap().is_empty());
    for invalid in ["../outside", "", "012345678901234567890123456789012g"] {
        assert!(library.character(invalid).is_err());
        assert!(matches!(library.audio(invalid), Err(AppError::NotFound(_))));
        assert!(matches!(
            library.delete_take(invalid),
            Err(AppError::NotFound(_))
        ));
        assert!(
            library
                .save_character(invalid.into(), character_input("test"))
                .is_err()
        );
    }
    for invalid in [" ".to_owned(), "あ".repeat(65)] {
        assert!(
            library
                .save_character(new_id(), character_input(&invalid))
                .is_err()
        );
    }
    assert!(
        library
            .save_character(new_id(), character_input(&"あ".repeat(64)))
            .is_ok()
    );
}
