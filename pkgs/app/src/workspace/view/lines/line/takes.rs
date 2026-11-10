use crate::domain::{AudioSource, Take};
use crate::workspace::protocol::Command;
use crate::workspace::view::Studio;
use crate::workspace::view::appearance::{blend, tool};
use gpui_kit::base::Radio;
use gpui_kit::component::Disableable;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(in crate::workspace::view) fn take_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let takes = self
            .state
            .snapshot
            .editor
            .lines
            .iter()
            .find(|line| Some(&line.id) == self.state.selected.as_ref())
            .map(|line| line.takes.as_slice())
            .unwrap_or_default();
        div()
            .w(relative(0.5))
            .flex_shrink_0()
            .min_w_0()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .when(takes.is_empty(), |el| {
                el.child(
                    div()
                        .text_xs()
                        .text_color(p.muted)
                        .child("音声を生成すると、ここにテイクが表示されます。"),
                )
            })
            .children(
                takes
                    .iter()
                    .filter_map(|id| self.state.snapshot.takes.iter().find(|take| &take.id == id))
                    .enumerate()
                    .map(|(i, take)| self.take_item(i, take, cx)),
            )
            .into_any_element()
    }

    fn take_item(&self, i: usize, take: &Take, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();

        let id = take.id.clone();
        let select_id = id.clone();
        let line_id = self.state.selected.clone().unwrap_or_default();
        let selected = self.state.snapshot.editor.lines.iter().any(|line| {
            Some(&line.id) == self.state.selected.as_ref() && line.take_id.as_ref() == Some(&id)
        });
        let disabled = self.state.busy;
        let tooltip = format!(
            "テイク {} · シード {} · {} ステップ · {:.1}秒で生成",
            i + 1,
            take.conditions.seed,
            take.conditions.num_steps,
            take.elapsed_ms as f64 / 1000.
        );
        let select = cx.listener(move |this, checked: &bool, window, cx| {
            if this.state.select_take(&select_id, *checked) {
                this.playback = None;
                this.audio_id = None;
                this.sync_fields(window, cx);
                this.send(Command::SelectTake {
                    line_id: line_id.clone(),
                    take_id: checked.then(|| select_id.clone()),
                });
                cx.notify();
            }
        });
        div()
            .flex()
            .items_center()
            .gap_2()
            .min_w_0()
            .child(
                Radio::new(("select-take", i))
                    .accessibility_label(format!("テイク {}を選択", i + 1))
                    .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
                    .size(px(22.))
                    .flex_shrink_0()
                    .rounded_full()
                    .border_2()
                    .border_color(if selected { self.accent() } else { p.muted })
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(disabled, |el| el.opacity(0.5))
                    .when(!disabled, |el| el.cursor_pointer())
                    .when(selected, |el| {
                        el.child(div().size(px(10.)).rounded_full().bg(self.accent()))
                    })
                    .checked(selected)
                    .disabled(disabled)
                    .on_change(move |checked, _, window, cx| select(&checked, window, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_1()
                    .border_1()
                    .border_color(if selected { self.accent() } else { p.line })
                    .bg(if selected {
                        blend(self.accent(), p.panel, 0.12)
                    } else {
                        p.panel
                    })
                    .rounded(px(8.))
                    .px_2()
                    .py_1()
                    .child(self.audio_player_controls(
                        ("take", i),
                        AudioSource::Take(id),
                        take.audio_seconds,
                        false,
                        cx,
                    ))
                    .child(self.take_actions(i, take, cx)),
            )
            .into_any_element()
    }

    fn take_actions(&self, i: usize, take: &Take, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.state.busy || self.state.status.id.is_none();
        let export = take.id.clone();
        let delete = take.id.clone();
        let copy = take.clone();
        div()
            .flex()
            .flex_shrink_0()
            .gap_1()
            .child(
                tool(("export", i), "download", "WAVを保存")
                    .disabled(disabled)
                    .when(disabled, |el| el.cursor_default())
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.export(AudioSource::Take(export.clone()), w, cx)
                    })),
            )
            .child(
                tool(("copy-take", i), "plus", "台本に追加")
                    .disabled(disabled)
                    .when(disabled, |el| el.cursor_default())
                    .on_click(cx.listener(move |this, _, w, cx| {
                        if this.state.copy_take(&copy) {
                            this.edited(w, cx);
                        }
                    })),
            )
            .child(
                tool(("delete-take", i), "trash", "音声を削除")
                    .disabled(disabled)
                    .when(disabled, |el| el.cursor_default())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.playback = None;
                        this.audio_id = None;
                        this.send(Command::DeleteTake(delete.clone()));
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
