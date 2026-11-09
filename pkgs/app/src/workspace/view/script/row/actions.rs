use crate::domain::{AudioSource, ScriptRow};
use crate::workspace::state::RowAction;
use crate::workspace::view::Studio;
use crate::workspace::view::appearance::tool;
use gpui_kit::component::Disableable;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(in crate::workspace::view) fn script_row_actions(
        &self,
        i: usize,
        row: &ScriptRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let disabled = self.state.busy || self.state.draft.is_some();
        div()
            .flex()
            .children(
                [
                    (RowAction::InsertAbove, 0, "plus", "上に追加"),
                    (RowAction::Duplicate, 2, "copy", "複製"),
                    (RowAction::Delete, 3, "trash", "削除"),
                ]
                .into_iter()
                .map(|(action, key, icon, label)| {
                    let id = row.id.clone();
                    let disabled = disabled
                        || (matches!(action, RowAction::Delete)
                            && self.state.snapshot.editor.rows.len() <= 1);
                    tool(("row-action", i * 4 + key), icon, label)
                        .size(px(40.))
                        .rounded_none()
                        .disabled(disabled)
                        .when(disabled, |el| el.cursor_default())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this.state.edit_row(&id, action) {
                                this.edited(window, cx);
                                this.fields
                                    .text
                                    .update(cx, |state, cx| state.focus(window, cx));
                            }
                        }))
                }),
            )
            .into_any_element()
    }

    pub(in crate::workspace::view) fn script_play_button(
        &self,
        i: usize,
        row: &ScriptRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let source = row
            .take_id
            .as_ref()
            .filter(|id| {
                self.state
                    .snapshot
                    .takes
                    .iter()
                    .any(|take| &take.id == *id && take.row_id == row.id)
            })
            .map(|id| AudioSource::Take(id.clone()));
        let playing = source.is_some()
            && self.audio_id == source
            && self
                .playback
                .as_ref()
                .is_some_and(|player| player.playing());
        let disabled = self.state.busy || source.is_none();
        tool(
            ("row-play", i),
            if playing { "pause" } else { "play" },
            if source.is_none() {
                "音声を生成すると再生できます"
            } else {
                "再生 / 一時停止"
            },
        )
        .size(px(40.))
        .rounded_none()
        .disabled(disabled)
        .when(disabled, |el| el.cursor_default())
        .on_click(cx.listener(move |this, _, _, cx| {
            if let Some(source) = &source {
                this.play(source.clone());
                cx.notify();
            }
        }))
        .into_any_element()
    }
}
