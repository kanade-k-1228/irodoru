use super::appearance::{Palette, blend, button, character_color, icon, tool};
use super::*;
use gpui_kit::component::{
    Sizable,
    scroll::{Scrollbar, ScrollbarMode},
};

impl Studio {
    pub(super) fn palette(&self) -> Palette {
        Palette::new(self.settings.light)
    }
    pub(super) fn accent(&self) -> Hsla {
        let color = self
            .draft
            .as_ref()
            .and_then(|d| d.input.color.as_deref())
            .or_else(|| {
                self.snapshot
                    .characters
                    .iter()
                    .find(|c| c.id == self.snapshot.editor.active_character)
                    .map(|c| c.color.as_str())
            });
        character_color(color, self.palette().accent)
    }
    pub(super) fn can_generate(&self) -> bool {
        self.service
            .as_ref()
            .is_some_and(|s| matches!(s.status(), InferenceStatus::Ready))
            && self.status.id.is_some()
    }
    fn remove_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.draft.is_some() {
            return;
        }
        if let Some(id) = self.selected.clone() {
            self.selected = editor::remove(&mut self.snapshot.editor, &id);
            self.snapshot.takes.retain(|t| t.row_id != id);
            self.autosave();
            self.sync_fields(window, cx);
        }
        cx.notify();
    }
    fn avatar(&self, character: Option<&Character>, size: f32) -> AnyElement {
        let p = self.palette();
        let color = character_color(character.map(|c| c.color.as_str()), p.accent);
        div()
            .size(px(size))
            .flex_shrink_0()
            .rounded_full()
            .border_1()
            .border_color(blend(color, p.line, 0.65))
            .bg(blend(color, p.panel, 0.18))
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .text_sm()
            .child(
                if let Some(image) = character_icon(character.and_then(|c| c.icon.as_deref())) {
                    img(image).size_full().into_any_element()
                } else {
                    div()
                        .child(
                            character
                                .and_then(|c| c.name.chars().next())
                                .map(|c| c.to_string())
                                .unwrap_or_else(|| "?".into()),
                        )
                        .into_any_element()
                },
            )
            .into_any_element()
    }
    pub(super) fn controls(&self, _cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.busy || self.status.id.is_none();
        div()
            .flex()
            .flex_col()
            .gap_0()
            .line_height(px(16.))
            .p_3()
            .pt(px(6.))
            .text_xs()
            .text_color(p.muted)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .mb_1()
                    .child("長さ（倍率）")
                    .child(
                        div()
                            .text_color(p.ink)
                            .border_b_1()
                            .border_color(p.line)
                            .px_1()
                            .child(format!("{:.2}×", self.settings_value().duration_scale)),
                    ),
            )
            .child(
                Slider::new(&self.sliders[0])
                    .bg(self.accent())
                    .text_color(self.accent())
                    .h(px(16.))
                    .flex_none()
                    .disabled(disabled),
            )
            .child(div().flex().justify_between().child("速く").child("遅く"))
            .into_any_element()
    }
    pub(super) fn advanced_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.busy || self.status.id.is_none();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .px_3()
            .pb_3()
            .text_xs()
            .text_color(p.muted)
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div().flex_1().min_w_0().child("シード").child(
                            Input::new(&self.fields.seed)
                                .small()
                                .bg(p.control)
                                .disabled(disabled),
                        ),
                    )
                    .child(
                        div().flex_1().min_w_0().child("ステップ数").child(
                            Input::new(&self.fields.steps)
                                .small()
                                .bg(p.control)
                                .disabled(disabled),
                        ),
                    ),
            )
            .children(
                self.sliders
                    .iter()
                    .skip(1)
                    .zip(["CFG 文", "CFG 声", "CFG 説明"])
                    .map(|(state, label)| {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div().flex().justify_between().child(label).child(
                                    div()
                                        .text_color(p.ink)
                                        .child(state.read(cx).value().to_string()),
                                ),
                            )
                            .child(
                                Slider::new(state)
                                    .bg(self.accent())
                                    .text_color(self.accent())
                                    .disabled(disabled),
                            )
                    }),
            )
            .when(self.draft.is_none(), |el| {
                el.child(
                    Input::new(&self.fields.prompt)
                        .small()
                        .bg(p.control)
                        .disabled(disabled),
                )
            })
            .when(self.draft.is_none(), |el| {
                el.child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            button("reset-settings", "既定設定に戻す")
                                .disabled(disabled)
                                .on_click(cx.listener(|this, _, w, cx| {
                                    if let Some(r) = this.selected_row_mut() {
                                        r.settings = None;
                                    }
                                    this.autosave();
                                    this.sync_fields(w, cx);
                                    cx.notify();
                                })),
                        )
                        .child(
                            tool("duplicate-line", "copy", "行を複製")
                                .disabled(
                                    disabled || self.snapshot.editor.rows.len() >= editor::MAX_ROWS,
                                )
                                .on_click(cx.listener(|this, _, w, cx| this.add(true, w, cx))),
                        ),
                )
            })
            .when(self.draft.is_none(), |el| {
                el.child(
                    button("generate-all", "すべて生成")
                        .disabled(disabled || !self.can_generate())
                        .on_click(cx.listener(|this, _, w, cx| this.generate(true, false, w, cx))),
                )
            })
            .into_any_element()
    }
    fn line_editor(&self, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.busy || self.status.id.is_none();
        if self.selected.is_none() {
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
                                    if self.running.is_some() {
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
                        self.snapshot
                            .takes
                            .iter()
                            .any(|t| Some(&t.row_id) == self.selected.as_ref()),
                        |el| el.border_l_1().border_color(p.line),
                    )
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(
                        self.snapshot
                            .takes
                            .iter()
                            .filter(|t| Some(&t.row_id) == self.selected.as_ref())
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
                                                                || self.snapshot.editor.rows.len()
                                                                    >= editor::MAX_ROWS,
                                                        )
                                                        .on_click(cx.listener(
                                                            move |this, _, w, cx| {
                                                                let row = editor::row_from_take(
                                                                    &copy,
                                                                    &this.snapshot.characters,
                                                                );
                                                                let id = row.id.clone();
                                                                this.snapshot.editor.rows.push(row);
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
    pub(super) fn audio_player(
        &self,
        key: (&'static str, usize),
        id: String,
        preview: bool,
        duration: f64,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let active = self.audio_id.as_deref() == Some(format!("{preview}:{id}").as_str());
        let player = self.playback.as_ref().filter(|_| active);
        let position = player.map(|p| p.position()).unwrap_or(0.);
        let playing = player.is_some_and(|p| p.playing());
        let export = id.clone();
        div()
            .flex()
            .items_center()
            .gap_1()
            .text_xs()
            .text_color(p.muted)
            .child(
                tool(
                    (key.0, key.1 * 2),
                    if playing { "pause" } else { "play" },
                    "再生 / 一時停止",
                )
                .disabled(self.busy)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.play(id.clone(), preview);
                    cx.notify();
                })),
            )
            .child(div().w(px(36.)).text_right().child(time(position.into())))
            .child(if active {
                Slider::new(&self.seek).flex_1().into_any_element()
            } else {
                div()
                    .flex_1()
                    .h(px(4.))
                    .rounded_full()
                    .bg(p.control)
                    .child(
                        div()
                            .size(px(10.))
                            .mt(px(-3.))
                            .rounded_full()
                            .bg(self.accent()),
                    )
                    .into_any_element()
            })
            .child(div().w(px(36.)).child(time(duration)))
            .child(
                tool((key.0, key.1 * 2 + 1), "download", "WAVを保存")
                    .disabled(self.busy)
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.export(export.clone(), preview, w, cx)
                    })),
            )
            .into_any_element()
    }
    fn characters(&self, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.busy || self.draft.is_some() || self.status.id.is_none();
        let active = self
            .snapshot
            .characters
            .iter()
            .find(|c| c.id == self.snapshot.editor.active_character)
            .cloned();
        div()
            .w(px(if narrow { 56. } else { 208. }))
            .flex_shrink_0()
            .min_h_0()
            .border_r_1()
            .border_color(p.line)
            .bg(p.surface)
            .flex()
            .flex_col()
            .child(
                div()
                    .id("characters")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(self.snapshot.characters.iter().enumerate().map(|(i, c)| {
                        let id = c.id.clone();
                        let selected = self
                            .draft
                            .as_ref()
                            .and_then(|d| d.id.as_ref())
                            .unwrap_or(&self.snapshot.editor.active_character)
                            == &id;
                        let color = character_color(Some(&c.color), p.accent);
                        div()
                            .id(("character", i))
                            .relative()
                            .min_h(px(44.))
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .border_1()
                            .border_color(if selected { p.line } else { p.surface })
                            .border_l(px(2.))
                            .when(selected, |el| el.bg(p.panel))
                            .child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .top_0()
                                    .bottom_0()
                                    .w(px(2.))
                                    .bg(if selected { color } else { p.surface }),
                            )
                            .text_color(if selected { p.ink } else { p.muted })
                            .cursor_pointer()
                            .hover(|s| s.bg(p.control))
                            .child(self.avatar(Some(c), 32.))
                            .when(!narrow, |el| {
                                el.child(div().text_xs().overflow_hidden().child(c.name.clone()))
                            })
                            .when(!disabled, |el| {
                                el.on_click(cx.listener(move |this, _, w, cx| {
                                    this.snapshot.editor.active_character = id.clone();
                                    match editor::insert(&mut this.snapshot.editor, None, false) {
                                        Ok(id) => this.select(id, w, cx),
                                        Err(e) => {
                                            this.error = Some(e.to_string());
                                            cx.notify();
                                        }
                                    }
                                }))
                            })
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap_1()
                    .when(narrow, |el| el.flex_col().items_center())
                    .px_2()
                    .py_1()
                    .border_t_1()
                    .border_color(p.line)
                    .child(
                        tool("add-character", "plus", "キャラを追加")
                            .size(px(32.))
                            .disabled(disabled)
                            .on_click(
                                cx.listener(|this, _, w, cx| this.edit_character(None, w, cx)),
                            ),
                    )
                    .child(
                        tool("edit-character", "edit", "キャラを編集")
                            .size(px(32.))
                            .disabled(disabled || active.is_none())
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.edit_character(active.clone(), w, cx)
                            })),
                    ),
            )
            .into_any_element()
    }
    fn script(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.busy || self.draft.is_some();
        div()
            .flex_1()
            .min_h_0()
            .mx_3()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .id("script")
                            .flex_1()
                            .min_w_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.script_scroll)
                            .p_1()
                            .children(self.snapshot.editor.rows.iter().enumerate().map(
                                |(i, row)| {
                                    let id = row.id.clone();
                                    let select = id.clone();
                                    let target = id.clone();
                                    let insert = id.clone();
                                    let selected = self.selected.as_ref() == Some(&id);
                                    let running = self.running.as_ref() == Some(&id);
                                    let character = self
                                        .snapshot
                                        .characters
                                        .iter()
                                        .find(|c| c.id == row.character_id);
                                    let last = i + 1 == self.snapshot.editor.rows.len();
                                    let color = character_color(
                                        character.map(|c| c.color.as_str()),
                                        p.accent,
                                    );
                                    div()
                                        .id(("row", i))
                                        .relative()
                                        .rounded(px(8.))
                                        .px_1()
                                        .py_1()
                                        .when(!last, |el| el.pb(px(24.)))
                                        .when(selected, |el| el.bg(blend(color, p.panel, 0.06)))
                                        .when(running, |el| el.border_1().border_color(p.accent))
                                        .child(
                                            div()
                                                .flex()
                                                .items_start()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .id(("drag", i))
                                                        .w(px(16.))
                                                        .h(px(28.))
                                                        .flex_shrink_0()
                                                        .flex()
                                                        .items_center()
                                                        .text_color(p.muted.opacity(0.4))
                                                        .cursor_move()
                                                        .child(icon("grip"))
                                                        .when(!disabled, |el| {
                                                            el.on_drag(
                                                                DragRow(id.clone()),
                                                                |drag, _, _, cx| {
                                                                    cx.new(|_| drag.clone())
                                                                },
                                                            )
                                                        }),
                                                )
                                                .child(
                                                    div()
                                                        .relative()
                                                        .w(px(28.))
                                                        .flex_shrink_0()
                                                        .child(
                                                            div()
                                                                .absolute()
                                                                .left(px(14.))
                                                                .top(px(15.))
                                                                .w(px(1.))
                                                                .h(px(if last { 0. } else { 42. }))
                                                                .bg(p.line),
                                                        )
                                                        .child(self.avatar(character, 28.)),
                                                )
                                                .child(
                                                    div()
                                                        .id(("speech", i))
                                                        .line_height(px(24.))
                                                        .relative()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .min_h(px(28.))
                                                        .rounded(px(8.))
                                                        .border_1()
                                                        .border_color(if selected {
                                                            p.surface
                                                        } else {
                                                            p.line
                                                        })
                                                        .bg(if selected {
                                                            blend(p.control, rgb(0).into(), 0.78)
                                                        } else {
                                                            p.control
                                                        })
                                                        .px_2()
                                                        .py(px(2.))
                                                        .pr(if selected { px(36.) } else { px(8.) })
                                                        .text_sm()
                                                        .text_color(if selected {
                                                            p.ink
                                                        } else if row.text.is_empty() {
                                                            p.muted
                                                        } else {
                                                            p.ink
                                                        })
                                                        .cursor_pointer()
                                                        .hover(|s| s.bg(p.hover))
                                                        .child(if row.text.is_empty() {
                                                            "セリフを入力".to_owned()
                                                        } else {
                                                            row.text.clone()
                                                        })
                                                        .on_click(cx.listener(
                                                            move |this, _, w, cx| {
                                                                this.select(select.clone(), w, cx);
                                                                this.fields
                                                                    .text
                                                                    .update(cx, |state, cx| {
                                                                        state.focus(w, cx)
                                                                    });
                                                            },
                                                        )),
                                                ),
                                        )
                                        .when(selected, |el| {
                                            el.child(
                                                div().absolute().right(px(8.)).top(px(6.)).child(
                                                    tool(("remove", i), "trash", "選択行を削除")
                                                        .bg(rgb(0xb52e42))
                                                        .text_color(rgb(0xffffff))
                                                        .disabled(
                                                            disabled
                                                                || self.snapshot.editor.rows.len()
                                                                    <= 1,
                                                        )
                                                        .on_click(cx.listener(|this, _, w, cx| {
                                                            this.remove_row(w, cx)
                                                        })),
                                                ),
                                            )
                                        })
                                        .when(!last, |el| {
                                            el.child(
                                                div()
                                                    .absolute()
                                                    .left(px(32.))
                                                    .bottom(px(2.))
                                                    .child(
                                                        tool(
                                                            ("insert", i),
                                                            "plus",
                                                            "ここにセリフを追加",
                                                        )
                                                        .size(px(20.))
                                                        .rounded_full()
                                                        .bg(p.panel)
                                                        .border_1()
                                                        .border_color(p.line)
                                                        .disabled(
                                                            disabled
                                                                || self.snapshot.editor.rows.len()
                                                                    >= editor::MAX_ROWS,
                                                        )
                                                        .on_click(cx.listener(
                                                            move |this, _, w, cx| {
                                                                this.selected =
                                                                    Some(insert.clone());
                                                                this.add(false, w, cx);
                                                            },
                                                        )),
                                                    ),
                                            )
                                        })
                                        .when_some(
                                            self.row_errors.get(&id).cloned(),
                                            |el, error| {
                                                el.child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(p.warning)
                                                        .child(error),
                                                )
                                            },
                                        )
                                        .on_drop(cx.listener(move |this, drag: &DragRow, w, cx| {
                                            if !this.busy && this.draft.is_none() {
                                                editor::reorder(
                                                    &mut this.snapshot.editor.rows,
                                                    &drag.0,
                                                    &target,
                                                    false,
                                                );
                                                this.select(drag.0.clone(), w, cx);
                                            }
                                        }))
                                },
                            )),
                    )
                    .child(
                        div()
                            .w(px(24.))
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .items_center()
                            .child(
                                tool("scroll-top", "up", "台本の先頭へ")
                                    .size(px(24.))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.script_scroll.set_offset(point(px(0.), px(0.)));
                                        cx.notify();
                                    })),
                            )
                            .child(div().relative().flex_1().w(px(24.)).child(
                                if self.script_scroll.max_offset().y <= px(0.) {
                                    div()
                                        .w(px(4.))
                                        .h_full()
                                        .mx_auto()
                                        .rounded_full()
                                        .child(
                                            div()
                                                .w(px(12.))
                                                .h(px(40.))
                                                .ml(px(-4.))
                                                .rounded(px(6.))
                                                .bg(if self.settings.light {
                                                    rgb(0x9ba8b8)
                                                } else {
                                                    rgb(0x397cae)
                                                }),
                                        )
                                        .into_any_element()
                                } else {
                                    Scrollbar::vertical(&self.script_scroll)
                                        .viewport_from_layout()
                                        .mode(ScrollbarMode::Always)
                                        .styles(|s| {
                                            s.track(|s| s.bg(transparent_black()).width(px(4.)))
                                                .thumb(|s| {
                                                    s.bg(if self.settings.light {
                                                        rgb(0x9ba8b8)
                                                    } else {
                                                        rgb(0x397cae)
                                                    })
                                                    .width(px(12.))
                                                })
                                        })
                                        .into_any_element()
                                },
                            ))
                            .child(
                                tool("scroll-bottom", "down", "台本の末尾へ")
                                    .size(px(24.))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.script_scroll.scroll_to_bottom();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .when(self.running.is_some(), |el| {
                el.child(
                    div().p_1().child(
                        button(
                            "stop",
                            if self.stopping {
                                "停止待ち"
                            } else {
                                "停止"
                            },
                        )
                        .disabled(self.stopping)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(s) = &this.service {
                                s.stop();
                                this.stopping = true;
                            }
                            cx.notify();
                        })),
                    ),
                )
            })
            .into_any_element()
    }
}
impl Render for Studio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let narrow = window.viewport_size().width < px(640.);
        let p = self.palette();
        let inference = self
            .service
            .as_ref()
            .map(|s| match s.status() {
                InferenceStatus::Loading => "anime · モデルを読み込み中".to_owned(),
                InferenceStatus::Ready => "anime · 生成できます".into(),
                InferenceStatus::Generating => "anime · 生成中".into(),
                InferenceStatus::Error { error } => format!("モデルエラー: {error}"),
            })
            .unwrap_or_default();
        let status = match self.status.state.as_str() {
            "saved" => "保存済み",
            "saving" => "保存中…",
            "error" => "保存失敗",
            _ => "",
        };
        let _ = &self.subscriptions;
        div()
            .relative()
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &crate::NewWindow, w, cx| this.menu_select(0, w, cx)))
            .on_action(cx.listener(|this, _: &crate::NewProject, w, cx| this.menu_select(1, w, cx)))
            .on_action(
                cx.listener(|this, _: &crate::OpenProject, w, cx| this.menu_select(2, w, cx)),
            )
            .on_action(
                cx.listener(|this, _: &crate::SaveProject, w, cx| this.menu_select(3, w, cx)),
            )
            .on_action(
                cx.listener(|this, _: &crate::Preferences, w, cx| this.menu_select(5, w, cx)),
            )
            .on_action(
                cx.listener(|this, _: &crate::CloseWindow, w, cx| this.menu_select(7, w, cx)),
            )
            .on_action(cx.listener(|this, _: &crate::Quit, w, cx| this.menu_select(7, w, cx)))
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(p.surface)
            .text_color(p.ink)
            .text_sm()
            .child(self.header(inference, status, window, cx))
            .when_some(self.error.clone(), |el, error| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .bg(p.warning_soft)
                        .text_color(p.warning)
                        .child(icon("alert"))
                        .child(div().flex_1().child(error))
                        .child(tool("dismiss-error", "close", "エラーを閉じる").on_click(
                            cx.listener(|this, _, _, cx| {
                                this.error = None;
                                cx.notify();
                            }),
                        )),
                )
            })
            .child(self.script(cx))
            .child(
                div()
                    .flex()
                    .h(px(
                        (f32::from(window.viewport_size().height) * 0.5).min(360.)
                    ))
                    .flex_shrink_0()
                    .min_h_0()
                    .border_t_1()
                    .border_color(p.line)
                    .bg(p.panel)
                    .child(self.characters(narrow, cx))
                    .child(if self.draft.is_some() {
                        self.character_editor(narrow, cx)
                    } else {
                        self.line_editor(narrow, cx)
                    }),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(p.line)
                    .bg(p.panel)
                    .px_2()
                    .py_1()
                    .text_xs()
                    .line_height(px(16.))
                    .text_color(p.muted)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(
                        self.status
                            .path
                            .clone()
                            .unwrap_or_else(|| "プロジェクト未選択".into()),
                    ),
            )
            .when(self.menu_open, |el| el.child(self.app_menu(cx)))
    }
}
fn time(seconds: f64) -> String {
    let seconds = seconds.max(0.) as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
pub(super) fn character_icon(data: Option<&str>) -> Option<std::sync::Arc<Image>> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data?.strip_prefix("data:image/png;base64,")?)
        .ok()?;
    Some(std::sync::Arc::new(Image::from_bytes(
        ImageFormat::Png,
        bytes,
    )))
}
