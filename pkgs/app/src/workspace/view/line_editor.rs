//! Line editor composition and generated take list.
use super::Studio;
use super::appearance::{blend, button, icon, tool};
use crate::domain::{AudioSource, Take};
use crate::workspace::protocol::Command;
use gpui_kit::base::Radio;
use gpui_kit::component::Disableable;
use gpui_kit::component::button::ButtonVariants;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(super) fn line_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("line-editor")
            .flex()
            .items_stretch()
            .w_full()
            .min_w_0()
            .child(self.take_list(cx))
            .child(self.line_form(cx))
            .into_any_element()
    }

    fn line_form(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .w(relative(0.5))
            .border_l_1()
            .border_color(self.palette().muted.opacity(0.45))
            .min_w_0()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .child(self.line_prompt())
            .child(self.speed_controls())
            .child(self.advanced_controls(cx))
            .child(self.line_generation_actions(cx))
            .into_any_element()
    }

    fn line_generation_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.status.id.is_none();
        div()
            .flex()
            .gap_2()
            .px_3()
            .pb_3()
            .child(
                button(
                    "generate-line",
                    if self.state.running.is_some() {
                        "生成中…"
                    } else {
                        "生成"
                    },
                )
                .primary()
                .bg(self.accent())
                .icon(icon("wand"))
                .flex_1()
                .disabled(
                    disabled
                        || !self.can_generate()
                        || self.fields.text.read(cx).value().trim().is_empty(),
                )
                .when(
                    disabled
                        || !self.can_generate()
                        || self.fields.text.read(cx).value().trim().is_empty(),
                    |el| el.cursor_default(),
                )
                .on_click(cx.listener(|this, _, w, cx| this.generate(false, false, w, cx))),
            )
            .child(
                tool("advanced", "settings", "詳細設定")
                    .h(px(32.))
                    .w(px(34.))
                    .border_1()
                    .border_color(if self.advanced { self.accent() } else { p.line })
                    .bg(if self.advanced { p.soft } else { p.control })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.advanced = !this.advanced;
                        cx.notify();
                    })),
            )
            .child(
                tool("random-line", "dice", "シードを変えて再生成")
                    .h(px(32.))
                    .w(px(34.))
                    .border_1()
                    .border_color(p.line)
                    .bg(p.control)
                    .disabled(disabled || !self.can_generate())
                    .when(disabled || !self.can_generate(), |el| el.cursor_default())
                    .on_click(cx.listener(|this, _, w, cx| this.generate(false, true, w, cx))),
            )
            .into_any_element()
    }

    fn take_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        div()
            .w(relative(0.5))
            .flex_shrink_0()
            .min_w_0()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .when(
                !self
                    .state
                    .snapshot
                    .takes
                    .iter()
                    .any(|take| Some(&take.row_id) == self.state.selected.as_ref()),
                |el| {
                    el.child(
                        div()
                            .text_xs()
                            .text_color(p.muted)
                            .child("音声を生成すると、ここにテイクが表示されます。"),
                    )
                },
            )
            .children(
                self.state
                    .snapshot
                    .takes
                    .iter()
                    .filter(|take| Some(&take.row_id) == self.state.selected.as_ref())
                    .enumerate()
                    .map(|(i, take)| self.take_item(i, take, cx)),
            )
            .into_any_element()
    }

    fn take_item(&self, i: usize, take: &Take, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();

        let id = take.id.clone();
        let select_id = id.clone();
        let row_id = take.row_id.clone();
        let selected = self.state.snapshot.editor.rows.iter().any(|row| {
            Some(&row.id) == self.state.selected.as_ref() && row.take_id.as_ref() == Some(&id)
        });
        let disabled = self.state.busy || self.state.draft.is_some();
        let tooltip = format!(
            "テイク {} · シード {} · {} ステップ · {:.1}秒で生成",
            i + 1,
            take.seed,
            take.num_steps,
            take.elapsed_ms as f64 / 1000.
        );
        let select = cx.listener(move |this, checked: &bool, window, cx| {
            if this.state.select_take(&select_id, *checked) {
                this.playback = None;
                this.audio_id = None;
                this.sync_fields(window, cx);
                this.send(Command::SelectTake {
                    row_id: row_id.clone(),
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
