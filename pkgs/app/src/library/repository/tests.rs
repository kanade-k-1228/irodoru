use super::*;

fn character_input(name: &str) -> CharacterInput {
    CharacterInput {
        name: name.into(),
        description: "落ち着いた声".into(),
        voice: None,
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
        filename: None,
        conditions: None,
        id: new_id(),
        row_id: "row".into(),
        character_id: Some(character.id.clone()),
        character_name: character.name,
        description: character.description,
        additional_prompt: String::new(),
        voice: None,
        text: "こんにちは".into(),
        model: "anime".into(),
        seed: 42,
        num_steps: 4,
        elapsed_ms: 100,
        audio_seconds: 1.0,
        created_at: 1,
    };
    library.save_take(&first, b"first wav").unwrap();
    let second = Take {
        id: new_id(),
        created_at: 2,
        ..first.clone()
    };
    library.save_take(&second, b"second wav").unwrap();
    library.delete_character(&character.id).unwrap();

    assert_eq!(library.takes().unwrap().len(), 2);
    assert_eq!(library.audio(&first.id).unwrap(), b"first wav");
    assert_eq!(library.audio(&second.id).unwrap(), b"second wav");
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
