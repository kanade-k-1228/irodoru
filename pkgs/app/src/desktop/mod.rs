//! Process and native window composition. Features request windows through events.
use crate::workspace::view::{Studio, WindowRequest, actions::*};
use gpui_kit::*;
#[cfg(target_os = "macos")]
mod macos_quit;
mod smoke;
use smoke::package_smoke_test;

pub(crate) fn open_studio(path: Option<std::path::PathBuf>, cx: &mut App) -> anyhow::Result<()> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1200.), px(850.)),
            cx,
        ))),
        window_min_size: Some(size(px(600.), px(500.))),
        titlebar: None,
        window_decorations: Some(WindowDecorations::Client),
        app_owns_titlebar_drag: true,
        ..Default::default()
    };
    gpui_kit::open_window(options, cx, move |window, cx| {
        window.set_window_title("irodoru");
        let studio = cx.new(|cx| Studio::new(path, window, cx));
        cx.subscribe(&studio, |studio, request, cx| match request {
            WindowRequest::New => {
                if let Err(error) = open_studio(None, cx) {
                    studio.update(cx, |studio, cx| studio.report_error(error.to_string(), cx));
                }
            }
        })
        .detach();
        studio
    })?;
    cx.activate(true);
    Ok(())
}
pub fn run() {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--smoke-test")
    {
        if let Err(error) = package_smoke_test() {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
        return;
    }
    let _instance = match crate::preferences::lock_instance() {
        Ok(lock) => lock,
        Err(error) => {
            rfd::MessageDialog::new()
                .set_title("irodoru")
                .set_description(format!("{error:#}"))
                .show();
            return;
        }
    };
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        if let Err(error) = crate::preferences::theme::initialize(cx) {
            eprintln!("{error:#}");
            cx.quit();
            return;
        }
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                #[cfg(target_os = "macos")]
                macos_quit::allow_exit();
                cx.quit();
            }
        })
        .detach();
        let [q, w, nw, n, o, s, prefs] = if cfg!(target_os = "macos") {
            [
                "cmd-q",
                "cmd-w",
                "cmd-shift-n",
                "cmd-n",
                "cmd-o",
                "cmd-s",
                "cmd-,",
            ]
        } else {
            [
                "ctrl-q",
                "ctrl-w",
                "ctrl-shift-n",
                "ctrl-n",
                "ctrl-o",
                "ctrl-s",
                "ctrl-,",
            ]
        };
        cx.bind_keys([
            KeyBinding::new(q, Quit, None),
            KeyBinding::new(w, CloseWindow, None),
            KeyBinding::new(nw, NewWindow, None),
            KeyBinding::new(n, NewProject, None),
            KeyBinding::new(o, OpenProject, None),
            KeyBinding::new(s, SaveProject, None),
            KeyBinding::new(prefs, Preferences, None),
        ]);
        cx.set_menus(vec![Menu {
            disabled: false,
            name: "irodoru".into(),
            items: vec![
                MenuItem::action("新規ウィンドウ", NewWindow),
                MenuItem::action("新規プロジェクト…", NewProject),
                MenuItem::action("プロジェクトを開く…", OpenProject),
                MenuItem::action("プロジェクトを保存", SaveProject),
                MenuItem::action("環境設定…", Preferences),
                MenuItem::action("ウィンドウを閉じる", CloseWindow),
            ],
        }]);
        if let Err(error) = open_studio(std::env::args_os().nth(1).map(Into::into), cx) {
            eprintln!("{error:#}");
            cx.quit();
        } else {
            #[cfg(target_os = "macos")]
            macos_quit::install(cx);
        }
    });
}
