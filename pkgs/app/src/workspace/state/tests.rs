use super::*;
use crate::domain::CharaInput;

#[test]
fn failed_chara_save_retains_local_values_and_project() {
    let mut state = WorkspaceState::default();
    state.status.id = Some("project".into());
    state.snapshot.charas.push(chara());
    state.snapshot.editor.active_chara = "chara".into();
    state.change(Field::Name, "変更後".into());
    state.apply(Event::Error("disk full".into()));
    assert_eq!(state.active_chara().unwrap().name, "変更後");
    assert!(state.save_chara().unwrap().is_passive());
    assert_eq!(state.status.id.as_deref(), Some("project"));
    assert_eq!(state.error.as_deref(), Some("disk full"));
}

#[test]
fn snapshot_reconciles_selection_and_selects_existing_chara() {
    let mut state = WorkspaceState {
        selected: Some("deleted".into()),
        ..Default::default()
    };
    let mut snapshot = Snapshot::default();
    snapshot.charas.push(chara());
    snapshot
        .editor
        .lines
        .push(crate::workspace::editor::new_line(
            "chara".into(),
            Default::default(),
        ));
    let id = snapshot.editor.lines[0].id.clone();
    assert!(matches!(
        state.apply(Event::Snapshot(snapshot)),
        Some(Effect::SyncInputs)
    ));
    assert_eq!(state.selected.as_deref(), Some(id.as_str()));
    assert_eq!(state.active_chara().unwrap().id, "chara");
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
fn close_waits_for_commands() {
    let mut state = WorkspaceState::default();
    assert!(state.close_error().is_none());
    state.busy = true;
    assert!(state.close_error().is_some());
    state.apply(Event::Done);
    assert!(state.close_error().is_none());
}

#[test]
fn line_and_chara_inputs_do_not_leak_settings() {
    let mut state = WorkspaceState::default();
    state
        .snapshot
        .editor
        .lines
        .push(crate::workspace::editor::new_line(
            "chara".into(),
            Default::default(),
        ));
    let id = state.snapshot.editor.lines[0].id.clone();
    assert!(state.select(id));
    assert!(state.change(Field::Seed, "42".into()));
    assert_eq!(state.settings_value().seed, "42");
    state.selected = None;
    assert!(!state.change(Field::Seed, "7".into()));
    assert!(state.settings_value().seed.is_empty());
    state.snapshot.charas.push(chara());
    state.snapshot.editor.active_chara = "chara".into();
    assert!(!state.change(Field::Name, "new chara".into()));
    state.preview_settings.seed = "9".into();
    assert_eq!(state.active_chara().unwrap().name, "new chara");
    assert_eq!(
        state.snapshot.editor.lines[0]
            .settings
            .as_ref()
            .unwrap()
            .seed,
        "42"
    );
    assert!(state.select(state.snapshot.editor.lines[0].id.clone()));
    assert!(state.change(Field::Text, "セリフ".into()));
    let before = state.active_chara().unwrap().clone();
    state.chara_locked = true;
    for field in [Field::Name, Field::Description, Field::Sample] {
        state.change(field, "blocked".into());
    }
    state.set_chara_color("#ffffff".into());
    state.set_chara_icon("blocked".into());
    state.toggle_chara_voice(Some("blocked".into()));
    state.change_preview_generation(GenerationField::Speed, 2.0);
    assert!(state.active_chara().unwrap() == &before);
    assert_eq!(state.preview_settings.duration_scale, 1.0);
    assert!(state.change(Field::Text, "ロック中もセリフは編集可能".into()));
    state.chara_locked = false;
    state.change(Field::Name, "unlocked".into());
    assert_eq!(state.active_chara().unwrap().name, "unlocked");
}

fn chara() -> crate::domain::Chara {
    CharaInput {
        name: "声".into(),
        description: "明るい声".into(),
        voice_ids: Vec::new(),
        color: None,
        sample_text: None,
        icon: None,
    }
    .normalize("chara".into())
    .unwrap()
}

#[test]
fn line_commands_preserve_selection_settings_and_audio_ownership() {
    let mut state = WorkspaceState::default();
    state.snapshot.charas.push(chara());
    assert!(state.add_line(false));
    state.change(Field::Seed, "17".into());
    let first = state.selected.clone().unwrap();
    state.change(Field::Text, "こんにちは".into());
    state.change(Field::Prompt, "小声で".into());
    state.selected_line_mut().unwrap().take_id = Some("audio".into());
    assert!(state.add_line(true));
    let duplicate = state.selected.clone().unwrap();
    assert_ne!(first, duplicate);
    let line = state.selected_line_mut().unwrap();
    assert_eq!(line.text, "こんにちは");
    assert_eq!(line.prompt.as_deref(), Some("小声で"));
    assert_eq!(line.settings.as_ref().unwrap().seed, "17");
    assert!(line.take_id.is_none());
    assert_eq!(state.move_line(None), Some(0));
    assert_eq!(state.selected.as_deref(), Some(duplicate.as_str()));
    assert!(state.remove_line());
    assert_eq!(state.selected.as_deref(), Some(first.as_str()));
    assert!(!state.remove_line());
    assert_eq!(
        state.snapshot.editor.lines[0].take_id.as_deref(),
        Some("audio")
    );
    assert!(state.insert_at(Some(&first), false));
    assert_eq!(state.snapshot.editor.lines.len(), 2);
    assert!(state.snapshot.editor.lines[1].text.is_empty());
    assert!(state.append_chara_line("another".into()));
    assert_eq!(state.snapshot.editor.lines[2].chara_id, "another");
    assert_eq!(state.snapshot.editor.active_chara, "another");
}

#[test]
fn busy_blocks_line_edits_and_preview_settings_remain_independent() {
    let mut state = WorkspaceState::default();
    state.add_line(false);
    let id = state.selected.clone();
    state.busy = true;
    assert!(!state.add_line(true));
    assert!(!state.remove_line());
    assert!(!state.change(Field::Text, "blocked".into()));
    assert!(!state.change_generation(GenerationField::Speed, 2.0));
    state.busy = false;
    state.change_generation(GenerationField::Speed, 2.0);
    assert_eq!(state.settings_value().duration_scale, 0.5);
    assert!(state.reset_line_settings());
    assert_eq!(state.settings_value().duration_scale, 1.0);
    state.change_preview_generation(GenerationField::Speed, 0.5);
    assert_eq!(state.preview_settings.duration_scale, 2.0);
    assert_eq!(state.settings_value().duration_scale, 1.0);
    assert_eq!(state.selected, id);
    assert!(state.change(Field::Text, "セリフ".into()));
}

#[test]
fn generation_validates_all_lines_before_returning_a_command() {
    use crate::workspace::protocol::Command;
    let mut state = WorkspaceState::default();
    state.snapshot.charas.push(chara());
    state.add_line(false);
    state.change(Field::Text, "一行目".into());
    state.add_line(true);
    state.change(Field::Text, "二行目".into());
    state.change(Field::Steps, "invalid".into());
    assert!(state.randomize_selected_line(9).unwrap());
    assert!(state.generate(true).is_err());
    assert_eq!(state.settings_value().seed, "9");
    state.reset_line_settings();
    let Some(Command::Generate(requests)) = state.generate(true).unwrap() else {
        panic!("expected generation command");
    };
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].request.text, "一行目");
    assert_eq!(requests[1].request.text, "二行目");
    let defaults = GenerationSettings::default();
    assert_eq!(requests[1].request.seed, None);
    assert_eq!(requests[1].request.num_steps, None);
    assert_eq!(requests[1].request.duration_scale, defaults.duration_scale);
    assert_eq!(requests[1].request.cfg_scale_text, defaults.cfg_text);
    assert_eq!(requests[1].request.cfg_scale_speaker, defaults.cfg_speaker);
    assert_eq!(requests[1].request.cfg_scale_caption, defaults.cfg_caption);
    assert!(state.randomize_selected_line(42).unwrap());
    let Some(Command::Generate(requests)) = state.generate(false).unwrap() else {
        panic!("expected selected line");
    };
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].request.seed, Some(42));
    assert!(
        state.snapshot.editor.lines[0]
            .settings
            .as_ref()
            .unwrap()
            .seed
            .is_empty()
    );
}

#[test]
fn insertion_gaps_add_and_duplicate_at_start_middle_and_end() {
    let mut state = WorkspaceState::default();
    state.snapshot.charas.push(chara());
    state.add_line(false);
    let source = state.selected.clone().unwrap();
    state.change(Field::Text, "元のセリフ".into());
    state.change(Field::Prompt, "小声".into());
    state.selected_line_mut().unwrap().take_id = Some("original-audio".into());
    state
        .selected_line_mut()
        .unwrap()
        .settings
        .as_mut()
        .unwrap()
        .seed = "42".into();
    assert!(state.insert_at(None, true));
    let copy = &state.snapshot.editor.lines[0];
    assert_ne!(copy.id, source);
    assert_eq!(state.selected.as_ref(), Some(&copy.id));
    assert_eq!(copy.text, "元のセリフ");
    assert_eq!(copy.prompt.as_deref(), Some("小声"));
    assert_eq!(copy.settings.as_ref().unwrap().seed, "42");
    assert!(copy.take_id.is_none());
    assert!(state.insert_at(Some(&source), true));
    assert_eq!(state.snapshot.editor.lines[2].text, "元のセリフ");
    assert!(state.insert_at(Some(&source), false));
    assert!(state.snapshot.editor.lines[2].text.is_empty());
    assert!(state.insert_at(None, false));
    assert!(state.snapshot.editor.lines[0].text.is_empty());
    let count = state.snapshot.editor.lines.len();
    let selected = state.selected.clone();
    assert!(!state.insert_at(Some("missing"), true));
    state.busy = true;
    assert!(!state.insert_at(None, false));
    assert!(!state.insert_at(None, true));
    assert_eq!(state.snapshot.editor.lines.len(), count);
    assert_eq!(state.selected, selected);
    assert_eq!(
        state
            .snapshot
            .editor
            .lines
            .iter()
            .find(|r| r.id == source)
            .unwrap()
            .take_id
            .as_deref(),
        Some("original-audio")
    );
}

#[test]
fn chara_duplication_preserves_settings_and_uses_unique_bounded_names() {
    use crate::workspace::protocol::Command;
    let mut state = WorkspaceState::default();
    state.status.id = Some("project".into());
    let mut original = chara();
    original.name = "あ".repeat(64);
    original.voice_ids = vec![crate::domain::new_id()];
    original.icon = Some("data:image/png;base64,icon".into());
    state.snapshot.editor.active_chara = original.id.clone();
    state.snapshot.charas.push(original.clone());
    for index in 1..=2 {
        let Some(Command::SaveChara { id, input }) = state.duplicate_chara() else {
            panic!("expected a new chara command");
        };
        assert!(id.is_none());
        assert_eq!(input.name, format!("{} - {index}", "あ".repeat(60)));
        assert_eq!(input.description, original.description);
        assert_eq!(input.voice_ids, original.voice_ids);
        assert_eq!(input.icon, original.icon);
        assert_eq!(input.color.as_ref(), Some(&original.color));
        assert_eq!(input.sample_text.as_ref(), Some(&original.sample_text));
        let duplicate = input.normalize(format!("copy-{index}")).unwrap();
        state.snapshot.editor.active_chara = duplicate.id.clone();
        state.snapshot.charas.push(duplicate);
    }
    assert!(state.snapshot.charas[0] == original);
    state.busy = true;
    assert!(state.duplicate_chara().is_none());
    state.busy = false;
    state.status.id = None;
    assert!(state.duplicate_chara().is_none());
}

#[test]
fn reference_selection_is_additive_and_can_remove_individual_voices() {
    let mut state = WorkspaceState::default();
    state.snapshot.charas.push(chara());
    state.snapshot.editor.active_chara = "chara".into();
    state.toggle_chara_voice(Some("first".into()));
    state.toggle_chara_voice(Some("second".into()));
    state.toggle_chara_voice(Some("third".into()));
    assert_eq!(
        state.active_chara().unwrap().voice_ids,
        ["first", "second", "third"]
    );
    state.toggle_chara_voice(Some("second".into()));
    assert_eq!(state.active_chara().unwrap().voice_ids, ["first", "third"]);
    state.busy = true;
    state.toggle_chara_voice(None);
    assert_eq!(state.active_chara().unwrap().voice_ids.len(), 2);
    state.busy = false;
    state.toggle_chara_voice(None);
    assert!(state.active_chara().unwrap().voice_ids.is_empty());
}
