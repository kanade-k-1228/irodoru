#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
//! Desktop application: process startup, GUI shell, project editing, storage and synthesis.
extern crate gpui_kit as gpui;

mod audio;
mod desktop;
mod domain;
mod files;
mod icons;
mod library;
mod preferences;
mod project;
mod synthesis;
mod workspace;

use clap::Parser;
#[cfg(target_os = "macos")]
use desktop::macos;
use desktop::{SharedInference, open_projects, open_studio, show_error, start_inference};
use gpui_kit::*;
use project::incoming::{self, Inbox};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};
use workspace::view::actions::*;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// 開くプロジェクトファイル (.iro)
    #[arg(value_name = "PROJECT")]
    projects: Vec<PathBuf>,
}

fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut args = std::env::args_os().skip(1);
        if args.next().is_some_and(|arg| arg == "--verify-windows-ml") {
            let Some(report) = args.next() else {
                std::process::exit(2);
            };
            let (gpu, embedded) = match args.next() {
                None => (false, false),
                Some(arg) if arg == "--gpu" => (true, false),
                Some(arg) if arg == "--embedded-cpu" => (false, true),
                _ => std::process::exit(2),
            };
            if args.next().is_some() {
                std::process::exit(2);
            }
            let result = irodoru_engine::windows_runtime::verify(gpu, embedded);
            let text = match &result {
                Ok(report) => report.clone(),
                Err(error) => format!("{error:#}\n"),
            };
            let written = std::fs::write(report, text).is_ok();
            std::process::exit(if result.is_ok() && written { 0 } else { 1 });
        }
    }
    let cli = Cli::parse();
    let paths =
        incoming::argument_paths(cli.projects, &std::env::current_dir().unwrap_or_default());
    let inbox = match Inbox::new() {
        Ok(inbox) => inbox,
        Err(error) => {
            show_error(error);
            return;
        }
    };
    let _instance = match preferences::lock_instance() {
        Ok(lock) => lock,
        Err(error) => {
            if !paths.is_empty()
                && matches!(
                    error.downcast_ref::<std::fs::TryLockError>(),
                    Some(std::fs::TryLockError::WouldBlock)
                )
            {
                if let Err(error) = inbox.forward(&paths) {
                    show_error(error);
                }
            } else {
                show_error(error);
            }
            return;
        }
    };
    let pending = Rc::new(RefCell::new(paths));
    let app = gpui_kit::application();
    let opened = pending.clone();
    app.on_open_urls(move |urls| opened.borrow_mut().extend(incoming::url_paths(urls)));
    app.run(move |cx| {
        gpui_kit::init(cx);
        cx.set_global(SharedInference(
            start_inference().map_err(|error| format!("{error:#}")),
        ));
        preferences::theme::initialize(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                #[cfg(target_os = "macos")]
                macos::allow_exit();
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
            KeyBinding::new("tab", MenuNext, Some("AppMenu")),
            KeyBinding::new("shift-tab", MenuPrevious, Some("AppMenu")),
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
        let paths = std::mem::take(&mut *pending.borrow_mut());
        if paths.is_empty() {
            if let Err(error) = open_studio(None, cx) {
                show_error(error);
                cx.quit();
                return;
            }
        } else {
            open_projects(paths, cx);
        }
        #[cfg(target_os = "macos")]
        macos::install(cx);
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let inbox = inbox.clone();
                let forwarded = cx
                    .background_executor()
                    .spawn(async move { inbox.drain() })
                    .await;
                let mut paths = std::mem::take(&mut *pending.borrow_mut());
                match forwarded {
                    Ok(batch) => paths.extend(batch),
                    Err(error) => {
                        // Do not show a repeating modal if a user's configuration disk disappears.
                        eprintln!("プロジェクトを開く要求を読み込めません: {error:#}");
                    }
                }
                if !paths.is_empty() {
                    cx.update(|cx| open_projects(paths, cx));
                }
            }
        })
        .detach();
    });
}
