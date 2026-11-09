use super::*;

fn draft() -> Draft {
    let input = CharacterInput {
        name: "編集中".into(),
        description: String::new(),
        voice_ids: Vec::new(),
        color: None,
        sample_text: None,
        icon: None,
    };
    Draft {
        id: None,
        original: input.clone(),
        input,
        preview: None,
        settings: GenerationSettings::default(),
    }
}

#[test]
fn failed_character_save_retains_draft_and_project() {
    let mut state = WorkspaceState {
        draft: Some(draft()),
        saving_draft: true,
        busy: true,
        ..Default::default()
    };
    state.status.id = Some("project".into());
    state.apply(Event::Error("disk full".into()));
    state.apply(Event::Done);
    assert_eq!(state.draft.as_ref().unwrap().input.name, "編集中");
    assert_eq!(state.status.id.as_deref(), Some("project"));
    assert!(!state.saving_draft);
    assert!(!state.busy);
    assert_eq!(state.error.as_deref(), Some("disk full"));
}

#[test]
fn snapshot_reconciles_selection_and_clears_only_saved_draft() {
    let mut state = WorkspaceState {
        draft: Some(draft()),
        selected: Some("deleted".into()),
        ..Default::default()
    };
    let mut snapshot = Snapshot::default();
    snapshot.editor.rows.push(crate::workspace::editor::new_row(
        "character".into(),
        Default::default(),
    ));
    let id = snapshot.editor.rows[0].id.clone();
    assert!(matches!(
        state.apply(Event::Snapshot(snapshot.clone())),
        Some(Effect::SyncInputs)
    ));
    assert_eq!(state.selected.as_deref(), Some(id.as_str()));
    assert!(state.draft.is_some());
    state.saving_draft = true;
    state.apply(Event::Snapshot(snapshot));
    assert!(state.draft.is_none());
    assert!(!state.saving_draft);
}

#[test]
fn stale_save_notifications_cannot_replace_idle_project() {
    let mut state = WorkspaceState::default();
    state.status.id = Some("current".into());
    let stale = ProjectStatus {
        id: Some("previous".into()),
        error: Some("disk full".into()),
        ..state.status.clone()
    };
    state.apply(Event::Status(stale.clone()));
    assert_eq!(state.status.id.as_deref(), Some("current"));
    assert!(state.error.is_none());
    state.busy = true;
    state.apply(Event::Status(stale));
    assert_eq!(state.status.id.as_deref(), Some("previous"));
    assert!(state.error.as_ref().unwrap().contains("disk full"));
}

#[test]
fn close_waits_for_commands_and_character_edits() {
    let mut state = WorkspaceState::default();
    assert!(state.close_error().is_none());
    state.busy = true;
    assert!(state.close_error().is_some());
    state.apply(Event::Done);
    state.draft = Some(draft());
    assert!(state.close_error().is_some());
    state.draft = None;
    assert!(state.close_error().is_none());
}

#[test]
fn input_changes_target_row_defaults_or_draft_without_leaking_settings() {
    let mut state = WorkspaceState::default();
    state
        .snapshot
        .editor
        .rows
        .push(crate::workspace::editor::new_row(
            "character".into(),
            Default::default(),
        ));
    let id = state.snapshot.editor.rows[0].id.clone();
    assert!(state.select(id));
    let defaults = state.snapshot.editor.preferences.seed.clone();
    assert!(state.change(Field::Seed, "42".into()));
    assert_eq!(state.settings_value().seed, "42");
    assert_eq!(state.snapshot.editor.preferences.seed, defaults);
    state.selected = None;
    state.change(Field::Seed, "7".into());
    assert_eq!(state.snapshot.editor.preferences.seed, "7");
    assert!(state.edit_character(None));
    assert!(!state.change(Field::Name, "new character".into()));
    state.change(Field::Seed, "9".into());
    assert_eq!(state.draft.as_ref().unwrap().input.name, "new character");
    assert_eq!(state.settings_value().seed, "9");
    assert_eq!(state.snapshot.editor.preferences.seed, "7");
    assert!(!state.select("blocked".into()));
}

fn character() -> crate::domain::Character {
    CharacterInput {
        name: "声".into(),
        description: "明るい声".into(),
        voice_ids: Vec::new(),
        color: None,
        sample_text: None,
        icon: None,
    }
    .normalize("character".into())
    .unwrap()
}

#[test]
fn row_commands_preserve_selection_settings_and_audio_ownership() {
    let mut state = WorkspaceState::default();
    state.snapshot.characters.push(character());
    state.snapshot.editor.preferences.seed = "17".into();
    assert!(state.add_row(false));
    let first = state.selected.clone().unwrap();
    state.change(Field::Text, "こんにちは".into());
    state.change(Field::Prompt, "小声で".into());
    state.selected_row_mut().unwrap().take_id = Some("audio".into());
    assert!(state.add_row(true));
    let duplicate = state.selected.clone().unwrap();
    assert_ne!(first, duplicate);
    let row = state.selected_row_mut().unwrap();
    assert_eq!(row.text, "こんにちは");
    assert_eq!(row.prompt.as_deref(), Some("小声で"));
    assert_eq!(row.settings.as_ref().unwrap().seed, "17");
    assert!(row.take_id.is_none());
    assert_eq!(state.move_row(None), Some(0));
    assert_eq!(state.selected.as_deref(), Some(duplicate.as_str()));
    assert!(state.remove_row());
    assert_eq!(state.selected.as_deref(), Some(first.as_str()));
    assert!(!state.remove_row());
    assert_eq!(
        state.snapshot.editor.rows[0].take_id.as_deref(),
        Some("audio")
    );
    assert!(state.insert_at(Some(&first), false));
    assert_eq!(state.snapshot.editor.rows.len(), 2);
    assert!(state.snapshot.editor.rows[1].text.is_empty());
    assert!(state.append_character_row("another".into()));
    assert_eq!(state.snapshot.editor.rows[2].character_id, "another");
    assert_eq!(state.snapshot.editor.active_character, "another");
}

#[test]
fn busy_and_draft_states_block_row_edits_and_keep_defaults() {
    let mut state = WorkspaceState::default();
    state.add_row(false);
    let id = state.selected.clone();
    state.busy = true;
    assert!(!state.add_row(true));
    assert!(!state.remove_row());
    assert!(!state.change(Field::Text, "blocked".into()));
    assert!(!state.change_generation(GenerationField::Speed, 2.0));
    state.busy = false;
    state.change_generation(GenerationField::Speed, 2.0);
    assert_eq!(state.settings_value().duration_scale, 0.5);
    assert_eq!(state.snapshot.editor.preferences.duration_scale, 1.0);
    assert!(state.reset_row_settings());
    assert_eq!(state.settings_value().duration_scale, 1.0);
    state.edit_character(None);
    assert!(!state.draft.as_ref().unwrap().is_dirty());
    assert!(!state.change_generation(GenerationField::Speed, 0.5));
    assert_eq!(state.settings_value().duration_scale, 2.0);
    state.change(Field::Name, "新しい声".into());
    assert!(state.draft.as_ref().unwrap().is_dirty());
    assert!(!state.add_row(false));
    assert!(!state.change(Field::Text, "hidden edit".into()));
    assert!(state.snapshot.editor.rows[0].text.is_empty());
    assert!(!state.remove_row());
    assert!(state.generate(true).unwrap().is_none());
    assert_eq!(state.selected, id);
    state.draft.as_mut().unwrap().preview = Some("draft-audio".into());
    assert!(
        matches!(state.discard_character(), Some(crate::workspace::protocol::Command::RemovePreview(id)) if id == "draft-audio")
    );
    assert_eq!(state.settings_value().duration_scale, 1.0);
    assert_eq!(state.snapshot.editor.preferences.duration_scale, 1.0);
}

#[test]
fn generation_validates_all_rows_before_returning_a_command() {
    use crate::workspace::protocol::Command;
    let mut state = WorkspaceState::default();
    state.snapshot.characters.push(character());
    state.add_row(false);
    state.change(Field::Text, "一行目".into());
    state.add_row(true);
    state.change(Field::Text, "二行目".into());
    state.change(Field::Steps, "invalid".into());
    assert!(state.randomize_selected_row(9).unwrap());
    assert!(state.generate(true).is_err());
    assert_eq!(state.settings_value().seed, "9");
    state.reset_row_settings();
    let Some(Command::Generate(requests)) = state.generate(true).unwrap() else {
        panic!("expected generation command");
    };
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].request.text, "一行目");
    assert_eq!(requests[1].request.text, "二行目");
    assert!(state.randomize_selected_row(42).unwrap());
    let Some(Command::Generate(requests)) = state.generate(false).unwrap() else {
        panic!("expected selected row");
    };
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].request.seed, Some(42));
    assert!(state.snapshot.editor.preferences.seed.is_empty());
}

#[test]
fn insertion_gaps_add_and_duplicate_at_start_middle_and_end() {
    let mut state = WorkspaceState::default();
    state.snapshot.characters.push(character());
    state.add_row(false);
    let source = state.selected.clone().unwrap();
    state.change(Field::Text, "元のセリフ".into());
    state.change(Field::Prompt, "小声".into());
    state.selected_row_mut().unwrap().take_id = Some("original-audio".into());
    state
        .selected_row_mut()
        .unwrap()
        .settings
        .as_mut()
        .unwrap()
        .seed = "42".into();
    assert!(state.insert_at(None, true));
    let copy = &state.snapshot.editor.rows[0];
    assert_ne!(copy.id, source);
    assert_eq!(state.selected.as_ref(), Some(&copy.id));
    assert_eq!(copy.text, "元のセリフ");
    assert_eq!(copy.prompt.as_deref(), Some("小声"));
    assert_eq!(copy.settings.as_ref().unwrap().seed, "42");
    assert!(copy.take_id.is_none());
    assert!(state.insert_at(Some(&source), true));
    assert_eq!(state.snapshot.editor.rows[2].text, "元のセリフ");
    assert!(state.insert_at(Some(&source), false));
    assert!(state.snapshot.editor.rows[2].text.is_empty());
    assert!(state.insert_at(None, false));
    assert!(state.snapshot.editor.rows[0].text.is_empty());
    let count = state.snapshot.editor.rows.len();
    let selected = state.selected.clone();
    assert!(!state.insert_at(Some("missing"), true));
    state.busy = true;
    assert!(!state.insert_at(None, false));
    assert!(!state.insert_at(None, true));
    assert_eq!(state.snapshot.editor.rows.len(), count);
    assert_eq!(state.selected, selected);
    assert_eq!(
        state
            .snapshot
            .editor
            .rows
            .iter()
            .find(|r| r.id == source)
            .unwrap()
            .take_id
            .as_deref(),
        Some("original-audio")
    );
}

#[test]
fn character_duplication_preserves_settings_and_uses_unique_bounded_names() {
    use crate::workspace::protocol::Command;
    let mut state = WorkspaceState::default();
    state.status.id = Some("project".into());
    let mut original = character();
    original.name = "あ".repeat(64);
    original.voice_ids = vec![crate::domain::new_id()];
    original.icon = Some("data:image/png;base64,icon".into());
    state.snapshot.editor.active_character = original.id.clone();
    state.snapshot.characters.push(original.clone());
    for index in 1..=2 {
        let Some(Command::SaveCharacter { id, input, preview }) = state.duplicate_character()
        else {
            panic!("expected a new character command");
        };
        assert!(id.is_none());
        assert!(preview.is_none());
        assert_eq!(input.name, format!("{} - {index}", "あ".repeat(60)));
        assert_eq!(input.description, original.description);
        assert_eq!(input.voice_ids, original.voice_ids);
        assert_eq!(input.icon, original.icon);
        assert_eq!(input.color.as_ref(), Some(&original.color));
        assert_eq!(input.sample_text.as_ref(), Some(&original.sample_text));
        let duplicate = input.normalize(format!("copy-{index}")).unwrap();
        state.snapshot.editor.active_character = duplicate.id.clone();
        state.snapshot.characters.push(duplicate);
    }
    assert!(state.snapshot.characters[0] == original);
    state.busy = true;
    assert!(state.duplicate_character().is_none());
    state.busy = false;
    state.draft = Some(draft());
    assert!(state.duplicate_character().is_none());
    state.draft = None;
    state.status.id = None;
    assert!(state.duplicate_character().is_none());
}

#[test]
fn reference_selection_is_additive_and_can_remove_individual_voices() {
    let mut state = WorkspaceState::default();
    assert!(state.edit_character(None));
    state.toggle_draft_voice(Some("first".into()));
    state.toggle_draft_voice(Some("second".into()));
    state.apply(Event::Voice("third".into()));
    state.apply(Event::Voice("third".into()));
    assert_eq!(
        state.draft.as_ref().unwrap().input.voice_ids,
        ["first", "second", "third"]
    );
    state.toggle_draft_voice(Some("second".into()));
    assert_eq!(
        state.draft.as_ref().unwrap().input.voice_ids,
        ["first", "third"]
    );
    state.busy = true;
    state.toggle_draft_voice(None);
    assert_eq!(state.draft.as_ref().unwrap().input.voice_ids.len(), 2);
    state.busy = false;
    state.toggle_draft_voice(None);
    assert!(state.draft.as_ref().unwrap().input.voice_ids.is_empty());
}
