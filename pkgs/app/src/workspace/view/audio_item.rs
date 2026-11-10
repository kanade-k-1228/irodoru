//! One complete audio row for both generated takes and reference recordings.
use super::Studio;
use super::appearance::{DisabledWithCursor, blend, tool};
use crate::domain::AudioSource;
use crate::workspace::protocol::Command;
use gpui_kit::base::{Checkbox, CheckboxState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

pub(super) struct AudioItem {
    pub key: (&'static str, usize),
    pub source: AudioSource,
    pub seconds: f64,
    pub selected: Option<bool>,
    pub label: String,
}

impl Studio {
    pub(super) fn audio_item(&self, item: AudioItem, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let (prefix, index) = item.key;
        let key = index * 8;
        let reference = matches!(item.source, AudioSource::Reference(_));
        let disabled = self.state.busy || self.state.status.id.is_none();
        let removal_disabled = disabled || (reference && self.state.chara_locked);
        let selected = item.selected.unwrap_or(false);
        let select_source = item.source.clone();
        let export_source = item.source.clone();
        let delete_source = item.source.clone();
        let select = cx.listener(move |this, checked: &CheckboxState, window, cx| {
            this.select_audio(
                &select_source,
                *checked == CheckboxState::Checked,
                window,
                cx,
            );
        });
        let selector_tooltip = item.label.clone();
        div()
            .id((prefix, key))
            .flex()
            .items_center()
            .gap_2()
            .min_w_0()
            .flex_shrink_0()
            .when_some(item.selected, |el, selected| {
                el.child(
                    Checkbox::new((prefix, key + 1))
                        .role(Role::RadioButton)
                        .accessibility_label(item.label.clone())
                        .tooltip(move |window, cx| {
                            Tooltip::new(selector_tooltip.clone()).build(window, cx)
                        })
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
            })
            .child(
                div()
                    .id((prefix, key + 5))
                    .flex()
                    .flex_wrap()
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
                    .tooltip(move |window, cx| Tooltip::new(item.label.clone()).build(window, cx))
                    .child(self.audio_player_controls(
                        (prefix, key + 2),
                        item.source,
                        item.seconds,
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .gap_1()
                            .child(
                                tool((prefix, key + 3), "download", "WAVを保存")
                                    .disabled_with_cursor(disabled)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.export(export_source.clone(), window, cx);
                                    })),
                            )
                            .child(
                                tool(
                                    (prefix, key + 4),
                                    "trash",
                                    if reference {
                                        "参照音声を削除"
                                    } else {
                                        "音声を削除"
                                    },
                                )
                                .disabled_with_cursor(removal_disabled)
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.remove_audio(&delete_source);
                                        cx.notify();
                                    },
                                )),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn select_audio(
        &mut self,
        source: &AudioSource,
        checked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let AudioSource::Take(id) = source else {
            return;
        };
        if self.state.select_take(id, checked) {
            self.stop_audio();
            self.sync_fields(window, cx);
            self.send(Command::SelectTake {
                line_id: self.state.selected.clone().unwrap_or_default(),
                take_id: checked.then(|| id.clone()),
            });
        }
        cx.notify();
    }

    fn remove_audio(&mut self, source: &AudioSource) {
        if self.state.busy {
            return;
        }
        match source {
            AudioSource::Take(id) => {
                self.stop_audio();
                self.send(Command::DeleteTake(id.clone()));
            }
            AudioSource::Reference(id) => {
                if self.state.remove_chara_voice(id) {
                    if self.audio_id.as_ref() == Some(source) {
                        self.stop_audio();
                    }
                    self.autosave_chara();
                }
            }
        }
    }
}
