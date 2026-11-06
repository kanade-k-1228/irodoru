use super::Studio;
use super::appearance::{blend, button, icon, tool};
use crate::workspace::editor;
use crate::workspace::protocol::Command;
use gpui_kit::component::Disableable;
use gpui_kit::component::button::ButtonVariants;
use gpui_kit::component::input::Textarea;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(super) fn remove_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.busy || self.state.draft.is_some() {
            return;
        }
        if let Some(id) = self.state.selected.clone() {
            self.state.selected = editor::remove(&mut self.state.snapshot.editor, &id);
            self.state.snapshot.takes.retain(|t| t.row_id != id);
            self.autosave();
            self.sync_fields(window, cx);
        }
        cx.notify();
    }
    pub(super) fn line_editor(&self, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.status.id.is_none();
        if self.state.selected.is_none() {
            return div()
                .flex_1()
                .text_center()
                .py_2()
                .text_xs()
                .text_color(p.muted)
                .child(icon("audio"))
                .child(div().child("台本の行を選択"))
                .into_any_element();
        }
        div()
            .id("line-editor")
            .flex()
            .when(narrow, |el| el.flex_col())
            .flex_1()
            .min_w_0()
            .overflow_y_scroll()
            .child(
                div()
                    .w(relative(if narrow { 1.0 } else { 0.5 }))
                    .min_w_0()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .child(
                        div().m_3().mb_0().child(
                            Textarea::new(&self.fields.text)
                                .h(px(70.))
                                .bg(p.control)
                                .disabled(disabled),
                        ),
                    )
                    .child(self.controls(cx))
                    .child(
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
                                .on_click(
                                    cx.listener(|this, _, w, cx| {
                                        this.generate(false, false, w, cx)
                                    }),
                                ),
                            )
                            .child(
                                tool("advanced", "settings", "詳細設定")
                                    .h(px(32.))
                                    .w(px(34.))
                                    .border_1()
                                    .border_color(p.line)
                                    .bg(p.control)
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
                                    .on_click(cx.listener(|this, _, w, cx| {
                                        this.generate(false, true, w, cx)
                                    })),
                            ),
                    )
                    .when(self.advanced, |el| el.child(self.advanced_controls(cx))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .when(
                        self.state
                            .snapshot
                            .takes
                            .iter()
                            .any(|t| Some(&t.row_id) == self.state.selected.as_ref()),
                        |el| el.border_l_1().border_color(p.line),
                    )
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(
                        self.state
                            .snapshot
                            .takes
                            .iter()
                            .filter(|t| Some(&t.row_id) == self.state.selected.as_ref())
                            .enumerate()
                            .map(|(i, t)| {
                                let id = t.id.clone();
                                let export = id.clone();
                                let delete = id.clone();
                                let copy = t.clone();
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .border_1()
                                            .border_color(self.accent())
                                            .bg(blend(self.accent(), p.panel, 0.12))
                                            .rounded(px(8.))
                                            .px_2()
                                            .py_1()
                                            .child(self.audio_player(
                                                ("take", i),
                                                id,
                                                false,
                                                t.audio_seconds,
                                                cx,
                                            )),
                                    )
                                    .when(self.advanced, |el| {
                                        el.child(div().text_xs().text_color(p.muted).child(
                                            format!(
                                                "seed {} · {} steps · {:.1}秒で生成",
                                                t.seed,
                                                t.num_steps,
                                                t.elapsed_ms as f64 / 1000.
                                            ),
                                        ))
                                        .child(
                                            div()
                                                .flex()
                                                .gap_1()
                                                .child(
                                                    tool(("export", i), "download", "WAVを保存")
                                                        .disabled(disabled)
                                                        .on_click(cx.listener(
                                                            move |this, _, w, cx| {
                                                                this.export(
                                                                    export.clone(),
                                                                    false,
                                                                    w,
                                                                    cx,
                                                                )
                                                            },
                                                        )),
                                                )
                                                .child(
                                                    tool(("copy-take", i), "plus", "台本に追加")
                                                        .disabled(
                                                            disabled
                                                                || self
                                                                    .state
                                                                    .snapshot
                                                                    .editor
                                                                    .rows
                                                                    .len()
                                                                    >= editor::MAX_ROWS,
                                                        )
                                                        .on_click(cx.listener(
                                                            move |this, _, w, cx| {
                                                                let row = editor::row_from_take(
                                                                    &copy,
                                                                    &this.state.snapshot.characters,
                                                                );
                                                                let id = row.id.clone();
                                                                this.state
                                                                    .snapshot
                                                                    .editor
                                                                    .rows
                                                                    .push(row);
                                                                this.select(id, w, cx);
                                                            },
                                                        )),
                                                )
                                                .child(
                                                    tool(("delete-take", i), "trash", "音声を削除")
                                                        .disabled(disabled)
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.playback = None;
                                                                this.audio_id = None;
                                                                this.send(
                                                                    Command::DeleteTake(
                                                                        delete.clone(),
                                                                    ),
                                                                    true,
                                                                );
                                                                cx.notify();
                                                            },
                                                        )),
                                                ),
                                        )
                                    })
                            }),
                    ),
            )
            .into_any_element()
    }
}
