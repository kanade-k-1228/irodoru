use irodoru::project::{Project, Session};

fn set_seed(project: &Project, seed: &str) {
    project
        .change(|session| {
            let mut editor = session.library.editor()?;
            editor.preferences.seed = seed.into();
            Ok(session.library.save_editor(&editor)?)
        })
        .unwrap();
}

fn saved_seed(path: &std::path::Path) -> String {
    Session::open(path.to_owned())
        .unwrap()
        .library
        .editor()
        .unwrap()
        .preferences
        .seed
}

#[test]
fn switching_and_dropping_flush_changes_before_releasing_the_archive() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.iro");
    let b = dir.path().join("b.iro");
    let mut first = Project::new(|_| {}).unwrap();
    let mut second = Project::new(|_| {}).unwrap();
    assert_eq!(first.status().unwrap().state, "closed");
    assert!(first.read(|_| Ok(())).is_err());
    assert!(first.change(|_| Ok(())).is_err());

    first.open(&a, true).unwrap();
    set_seed(&first, "7");
    assert!(second.open(&a, false).is_err());
    first.open(&b, true).unwrap();
    assert_eq!(saved_seed(&a), "7");
    second.open(&a, false).unwrap();
    set_seed(&first, "42");
    assert!(second.open(&b, false).is_err());

    drop(first);
    second.open(&b, false).unwrap();
    assert_eq!(saved_seed(&b), "42");
}

#[test]
fn failed_flush_preserves_the_active_session_and_its_reservation_until_retry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.iro");
    let backup = dir.path().join("backup.iro");
    let next = dir.path().join("next.iro");
    let mut project = Project::new(|_| {}).unwrap();
    project.open(&path, true).unwrap();
    project.flush().unwrap();
    let id = project.status().unwrap().id;

    std::fs::rename(&path, &backup).unwrap();
    std::fs::create_dir(&path).unwrap();
    set_seed(&project, "42");
    assert!(project.open(&next, true).is_err());
    assert_eq!(project.status().unwrap().id, id);
    assert_eq!(project.status().unwrap().state, "error");
    assert!(!next.exists());
    let mut other = Project::new(|_| {}).unwrap();
    assert!(other.open(&path, true).is_err());

    std::fs::remove_dir(&path).unwrap();
    std::fs::rename(&backup, &path).unwrap();
    project.open(&next, true).unwrap();
    assert_eq!(saved_seed(&path), "42");
    other.open(&path, false).unwrap();
}
