#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
extern crate gpui_kit as gpui;
#[cfg(target_os = "macos")]
mod macos_quit;
mod playback;
mod ui;
use gpui_kit::*;
gpui_kit::actions!(
    irodoru,
    [
        Quit,
        NewWindow,
        NewProject,
        OpenProject,
        SaveProject,
        Preferences,
        CloseWindow
    ]
);
pub(crate) struct AppTheme(pub bool);
pub(crate) struct ThemeWriter(pub irodoru_desktop::settings::SettingsWriter);
impl Global for ThemeWriter {}
impl Global for AppTheme {}
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
        cx.new(|cx| ui::Studio::new(path, window, cx))
    })?;
    cx.activate(true);
    Ok(())
}
fn main() {
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
    let _instance = match irodoru_desktop::settings::lock_instance() {
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
        match irodoru_desktop::settings::SettingsWriter::start() {
            Ok(writer) => cx.set_global(ThemeWriter(writer)),
            Err(error) => {
                eprintln!("{error:#}");
                cx.quit();
                return;
            }
        }
        cx.set_global(AppTheme(irodoru_desktop::settings::Settings::load().light));
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

// Run the shipped executable without a display or audio device. This exercises its
// actual runtime linkage and executable-relative model paths after package extraction.
fn package_smoke_test() -> anyhow::Result<()> {
    use irodoru_engine::{Engine, EngineConfig, SynthesisRequest};
    let path = irodoru_desktop::resources::models()?;
    let mut engine = Engine::load(&EngineConfig::anime(path.clone()))?;
    let job = SynthesisRequest {
        text: "こんにちは。".into(),
        seed: Some(42),
        num_steps: Some(1),
        seconds: Some(0.5),
        ..Default::default()
    }
    .try_into()?;
    let (conditions, output) = engine.synthesize(job)?;
    let wav = conditions.to_wav(&output.audio)?;
    let reader = hound::WavReader::new(std::io::Cursor::new(wav))?;
    anyhow::ensure!(
        reader.duration() > 0,
        "Packaged model generated empty audio"
    );
    println!(
        "Packaged offline synthesis passed: {} frames, models at {}",
        reader.duration(),
        path.display()
    );
    Ok(())
}
