use irodoru::project::{Project, Session};
#[test]
fn version_one_web_app_archive_opens_and_resaves_without_conversion() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("project.zip");
    std::fs::write(&path, include_bytes!("fixtures/legacy-project.zip")).unwrap();
    let mut project = Project::new(|_| {}).unwrap();
    project.open(&path, false).unwrap();
    let mut editor = project.read(|s| Ok(s.library.editor()?)).unwrap();
    assert_eq!(editor.rows[0].id, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    assert_eq!(editor.rows[0].prompt.as_deref(), Some("小声で"));
    assert_eq!(editor.rows[0].settings.as_ref().unwrap().seed, "42");
    assert_eq!(
        project.read(|s| Ok(s.library.characters()?)).unwrap()[0].name,
        "さくら"
    );
    editor.rows[0].text = "こんばんは。".into();
    project
        .change(|s| Ok(s.library.save_editor(&editor)?))
        .unwrap();
    project.flush().unwrap();
    let reopened = Session::open(path).unwrap();
    assert_eq!(
        reopened.library.editor().unwrap().rows[0].text,
        "こんばんは。"
    );
    assert_eq!(
        reopened.library.characters().unwrap()[0].id,
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );
}
