use super::*;

fn character_input(name: &str) -> CharacterInput {
    CharacterInput {
        name: name.into(),
        description: "落ち着いた声".into(),
        voice_ids: Vec::new(),
        icon: None,
        color: None,
        sample_text: None,
    }
}

#[test]
fn character_edits_preserve_existing_settings() {
    let dir = tempfile::tempdir().unwrap();
    let library = Library::open(dir.path()).unwrap();
    let mut input = character_input("さくら");
    input.color = Some("#ABCDEF".into());
    input.sample_text = Some("サンプル文".into());
    input.icon = Some("data:image/png;base64,iVBORw0KGgo=".into());
    let original = library.create_character(input).unwrap();
    library
        .update_character(original.id.clone(), character_input("更新後"))
        .unwrap();

    let reopened = Library::open(dir.path()).unwrap();
    let edited = reopened.character(&original.id).unwrap();
    assert_eq!(edited.name, "更新後");
    assert_eq!(edited.color, "#abcdef");
    assert_eq!(edited.sample_text, "サンプル文");
    assert_eq!(edited.icon, original.icon);
}

#[test]
fn regeneration_retains_takes_and_character_deletion_keeps_them() {
    let dir = tempfile::tempdir().unwrap();
    let library = Library::open(dir.path()).unwrap();
    let character = library.create_character(character_input("さくら")).unwrap();
    let first = Take {
        conditions: irodoru_engine::GenerationConditions {
            text: "こんにちは".into(),
            model: "anime".into(),
            seed: 42,
            num_steps: 4,
            ..crate::domain::Take::test_conditions()
        },
        id: new_id(),
        row_id: "row".into(),
        character_id: Some(character.id.clone()),
        character_name: character.name,
        additional_prompt: String::new(),
        voice_ids: Vec::new(),
        elapsed_ms: 100,
        audio_seconds: 1.0,
        created_at: 1,
    };
    let first_wav = irodoru_engine::Audio::new(vec![0.1; 10], 48000)
        .to_wav()
        .unwrap();
    library.save_take(&first, &first_wav).unwrap();
    let first_wav = library.audio(&first.id).unwrap();
    let second = Take {
        id: new_id(),
        created_at: 2,
        ..first.clone()
    };
    let second_wav = irodoru_engine::Audio::new(vec![0.2; 10], 48000)
        .to_wav()
        .unwrap();
    library.save_take(&second, &second_wav).unwrap();
    let second_wav = library.audio(&second.id).unwrap();
    library.delete_character(&character.id).unwrap();

    assert_eq!(library.takes().unwrap().len(), 2);
    assert_eq!(library.audio(&first.id).unwrap(), first_wav);
    assert_eq!(library.audio(&second.id).unwrap(), second_wav);
    library.delete_take(&second.id).unwrap();
    assert_eq!(library.takes().unwrap().len(), 1);
}

#[test]
fn invalid_names_and_paths_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let library = Library::open(dir.path()).unwrap();
    assert!(library.create_character(character_input(" ")).is_err());
    assert!(library.character("../outside").is_err());
    assert!(library.audio("../outside").is_err());
}

#[test]
fn failed_document_save_restores_script_files_and_keeps_the_previous_order() {
    let dir = tempfile::tempdir().unwrap();
    let library = Library::open(dir.path()).unwrap();
    let mut editor = crate::domain::EditorState::default();
    let mut row = crate::workspace::editor::new_row(String::new(), Default::default());
    row.text = "before".into();
    editor.rows.push(row);
    library.save_editor(&editor).unwrap();
    let project_path = dir.path().join("project.yaml");
    let document = std::fs::read(&project_path).unwrap();
    // Inject the failure after staging new scripts, when publishing project.yaml.
    std::fs::remove_file(&project_path).unwrap();
    std::fs::create_dir(&project_path).unwrap();
    editor.rows[0].text = "after".into();
    assert!(library.write_editor(&editor).is_err());
    std::fs::remove_dir(&project_path).unwrap();
    std::fs::write(&project_path, document).unwrap();
    assert_eq!(library.editor().unwrap().rows[0].text, "before");
    assert!(library.script_path("../escape").is_err());
    editor.rows.push(editor.rows[0].clone());
    assert!(library.save_editor(&editor).is_err());
    assert_eq!(library.editor().unwrap().rows.len(), 1);
}
