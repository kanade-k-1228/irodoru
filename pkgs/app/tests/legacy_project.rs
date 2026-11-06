use irodoru_desktop::project::{Saver, Session};
use std::sync::{Arc, Mutex};
#[test]
fn version_one_web_app_archive_opens_and_resaves_without_conversion() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("project.zip");
    std::fs::write(&path, include_bytes!("fixtures/legacy-project.zip")).unwrap();
    let mut session = Session::open(path.clone()).unwrap();
    let mut editor = session.library.editor().unwrap();
    assert_eq!(editor.rows[0].id, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    assert_eq!(editor.rows[0].prompt.as_deref(), Some("小声で"));
    assert_eq!(editor.rows[0].settings.as_ref().unwrap().seed, "42");
    assert_eq!(session.library.characters().unwrap()[0].name, "さくら");
    editor.rows[0].text = "こんばんは。".into();
    session.library.save_editor(&editor).unwrap();
    session.changed();
    let shared = Arc::new(Mutex::new(Some(session)));
    let saver = Saver::start(shared, |_| {}).unwrap();
    saver.changed().unwrap();
    saver.flush().unwrap();
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
