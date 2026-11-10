use super::*;

#[test]
fn removing_a_reference_is_character_local_and_cannot_toggle_it_back_on() {
    let mut state = WorkspaceState::default();
    let first = crate::domain::CharaInput {
        name: "キャラクター1".into(),
        description: String::new(),
        voice_ids: vec![crate::domain::new_id(), crate::domain::new_id()],
        color: None,
        icon: None,
    }
    .normalize(crate::domain::new_id())
    .unwrap();
    let mut second = first.clone();
    second.id = crate::domain::new_id();
    state.snapshot.editor.active_chara = first.id.clone();
    state.snapshot.charas = vec![first.clone(), second];
    let removed = &first.voice_ids[0];

    state.chara_locked = true;
    assert!(!state.remove_chara_voice(removed));
    state.chara_locked = false;
    state.busy = true;
    assert!(!state.remove_chara_voice(removed));
    assert_eq!(state.active_chara().unwrap().voice_ids, first.voice_ids);

    state.busy = false;
    assert!(state.remove_chara_voice(removed));
    assert!(!state.remove_chara_voice(removed));
    assert_eq!(
        state.active_chara().unwrap().voice_ids,
        first.voice_ids[1..]
    );
    assert_eq!(state.snapshot.charas[1].voice_ids, first.voice_ids);
}

#[test]
fn line_details_require_an_explicit_toggle_and_follow_the_editing_target() {
    let mut state = WorkspaceState::default();
    assert!(state.add_line(false));
    let first = state.selected.clone().unwrap();
    assert!(state.add_line(false));
    let second = state.selected.clone().unwrap();
    assert!(state.expanded_line.is_none());

    state.select(first.clone());
    assert!(state.expanded_line.is_none());
    assert!(state.toggle_line_details(first.clone()));
    assert_eq!(state.expanded_line.as_ref(), Some(&first));
    state.change(Field::Text, "編集中のセリフ".into());
    state.apply(Event::Snapshot(state.snapshot.clone()));
    assert_eq!(state.expanded_line.as_ref(), Some(&first));
    state.toggle_line_details(first.clone());
    assert!(state.expanded_line.is_none());

    state.toggle_line_details(second.clone());
    assert_eq!(state.selected.as_ref(), Some(&second));
    assert_eq!(state.expanded_line.as_ref(), Some(&second));
    state.select(first.clone());
    assert!(state.expanded_line.is_none());
    assert!(!state.toggle_line_details("missing".into()));
    assert_eq!(state.selected.as_ref(), Some(&first));

    state.toggle_line_details(first.clone());
    state.busy = true;
    assert!(!state.toggle_line_details(second));
    assert_eq!(state.expanded_line.as_ref(), Some(&first));
    state.busy = false;
    state.edit_line(&first, LineAction::Duplicate);
    assert!(state.expanded_line.is_none());
    state.toggle_line_details(first);
    assert!(state.remove_line());
    assert!(state.expanded_line.is_none());
    while let Some(id) = state.selected.clone() {
        assert!(state.toggle_line_details(id));
        assert!(state.remove_line());
        assert!(state.expanded_line.is_none());
    }
    assert!(state.snapshot.editor.lines.is_empty());
    assert!(!state.remove_line());
    assert!(state.add_line(false));
    assert!(state.selected.is_some());
}
