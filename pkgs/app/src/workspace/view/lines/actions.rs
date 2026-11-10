use crate::domain::{AudioSource, Line};
use crate::workspace::state::LineAction;
use crate::workspace::view::Studio;
use crate::workspace::view::appearance::DisabledWithCursor;
use crate::workspace::view::appearance::tool;
use gpui_kit::*;

impl Studio {
    pub(in crate::workspace::view) fn line_actions(
        &self,
        i: usize,
        line: &Line,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let disabled = self.state.busy;
        let id = line.id.clone();
        let expanded = self.state.expanded_line.as_ref() == Some(&id);
        let label = if expanded {
            "詳細を閉じる"
        } else {
            "詳細を開く"
        };
        div()
            .flex()
            .children(
                [
                    (LineAction::Duplicate, 2, "copy", "複製"),
                    (LineAction::Delete, 3, "trash", "削除"),
                ]
                .into_iter()
                .map(|(action, key, icon, label)| {
                    let id = line.id.clone();
                    tool(("line-action", i * 4 + key), icon, label)
                        .size(px(40.))
                        .rounded_none()
                        .disabled_with_cursor(disabled)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this.state.edit_line(&id, action) {
                                this.edited(window, cx);
                                this.fields
                                    .text
                                    .update(cx, |state, cx| state.focus(window, cx));
                            }
                        }))
                }),
            )
            .child(
                tool(
                    ("toggle-line-details", i),
                    if expanded {
                        "chevron-up"
                    } else {
                        "chevron-down"
                    },
                    label,
                )
                .size(px(40.))
                .rounded_none()
                .accessibility_label(label)
                .disabled_with_cursor(disabled)
                .on_click(cx.listener(move |this, _, window, cx| {
                    if this.state.toggle_line_details(id.clone()) {
                        this.sync_fields(window, cx);
                        cx.notify();
                    }
                })),
            )
            .into_any_element()
    }

    pub(in crate::workspace::view) fn line_play_button(
        &self,
        i: usize,
        line: &Line,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let source = line
            .take_id
            .as_ref()
            .filter(|id| {
                self.state
                    .snapshot
                    .takes
                    .iter()
                    .any(|take| &take.id == *id && line.takes.contains(&take.id))
            })
            .map(|id| AudioSource::Take(id.clone()));
        self.audio_play_button(("line-play", i), source, cx)
            .size(px(40.))
            .rounded_none()
            .into_any_element()
    }
}
