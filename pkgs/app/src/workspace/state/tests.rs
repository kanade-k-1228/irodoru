use super::*;

fn draft() -> Draft {
    let input = CharacterInput {
        name: "編集中".into(),
        description: String::new(),
        voice: None,
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
fn generation_events_clear_errors_and_finish_stop_request() {
    let mut state = WorkspaceState {
        busy: true,
        stopping: true,
        ..Default::default()
    };
    state.apply(Event::RowFailed("row".into(), "failed".into()));
    state.apply(Event::RowStarted("row".into()));
    assert!(state.row_errors.is_empty());
    assert_eq!(state.running.as_deref(), Some("row"));
    state.apply(Event::Done);
    assert!(!state.busy && !state.stopping && state.running.is_none());
}

#[test]
fn replacing_preview_requests_cleanup_without_io() {
    let mut state = WorkspaceState {
        draft: Some(draft()),
        ..Default::default()
    };
    assert!(state.apply(Event::Preview("first".into())).is_none());
    assert!(
        matches!(state.apply(Event::Preview("second".into())), Some(Effect::RemovePreview(id)) if id == "first")
    );
    assert_eq!(
        state.draft.as_ref().unwrap().preview.as_deref(),
        Some("second")
    );
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
    state.defaults = true;
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
