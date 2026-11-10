//! Persist GPUI's native window placement; let GPUI choose initial/cascaded bounds.
use anyhow::{Context as _, Result};
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use std::{cell::Cell, path::PathBuf, rc::Rc};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum Placement {
    Windowed(Bounds<Pixels>),
    Maximized(Bounds<Pixels>),
    Fullscreen(Bounds<Pixels>),
}

impl From<WindowBounds> for Placement {
    fn from(value: WindowBounds) -> Self {
        match value {
            WindowBounds::Windowed(bounds) => Self::Windowed(bounds),
            WindowBounds::Maximized(bounds) => Self::Maximized(bounds),
            WindowBounds::Fullscreen(bounds) => Self::Fullscreen(bounds),
        }
    }
}

impl From<Placement> for WindowBounds {
    fn from(value: Placement) -> Self {
        match value {
            Placement::Windowed(bounds) => Self::Windowed(bounds),
            Placement::Maximized(bounds) => Self::Maximized(bounds),
            Placement::Fullscreen(bounds) => Self::Fullscreen(bounds),
        }
    }
}

fn path() -> Result<PathBuf> {
    let dirs = crate::files::project_dirs().context("設定フォルダが見つかりません")?;
    Ok(dirs.config_dir().join("window.yaml"))
}

pub(super) fn restore(cx: &App) -> Option<(WindowBounds, DisplayId)> {
    // Additional windows use GPUI's cascade placement instead of overlapping.
    if !cx.windows().is_empty() {
        return None;
    }
    let saved: Placement = serde_yaml::from_slice(&std::fs::read(path().ok()?).ok()?).ok()?;
    let placement = WindowBounds::from(saved);
    let bounds = placement.get_bounds();
    if !valid(bounds) {
        return None;
    }
    // A removed display or changed layout falls back to GPUI's default placement.
    let display = cx.displays().into_iter().find(|display| {
        let visible = display.visible_bounds();
        visible.contains(&bounds.origin) && visible.contains(&bounds.bottom_right())
    })?;
    Some((placement, display.id()))
}

fn valid(bounds: Bounds<Pixels>) -> bool {
    [
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        bounds.size.height,
    ]
    .into_iter()
    .all(|value| f32::from(value).is_finite())
        && bounds.size.width >= px(600.)
        && bounds.size.height >= px(500.)
}

pub(super) fn remember<T: 'static>(window: &mut Window, cx: &mut Context<T>) {
    let placement = Rc::new(Cell::new(Placement::from(window.window_bounds())));
    let latest = placement.clone();
    cx.observe_window_bounds(window, move |_, window, _| {
        latest.set(window.window_bounds().into());
    })
    .detach();
    cx.on_release(move |_, _| {
        let saved = placement.get();
        if !valid(WindowBounds::from(saved).get_bounds()) {
            return;
        }
        let save = || -> Result<()> {
            let path = path()?;
            std::fs::create_dir_all(path.parent().context("設定の保存先が不正です")?)?;
            crate::files::atomic_bytes(&path, serde_yaml::to_string(&saved)?.as_bytes())?;
            Ok(())
        };
        if let Err(error) = save() {
            eprintln!("ウィンドウ位置を保存できません: {error:#}");
        }
    })
    .detach();
}
