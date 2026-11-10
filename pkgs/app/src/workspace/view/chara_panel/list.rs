use crate::domain::Chara;
use crate::workspace::view::Studio;
use crate::workspace::view::appearance::DisabledWithCursor;
use crate::workspace::view::appearance::icon_button;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(in crate::workspace::view) fn charas(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        div()
            .w(px(208.))
            .flex_shrink_0()
            .min_h_0()
            .border_r_1()
            .border_color(p.line)
            .bg(p.surface)
            .flex()
            .flex_col()
            .child(self.chara_list_wheel.probe())
            .child(
                div()
                    .id("charas")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(self.chara_list_wheel.handle())
                    .on_scroll(self.chara_list_wheel.listener(cx.entity_id()))
                    .p_1()
                    .children(
                        self.state
                            .snapshot
                            .charas
                            .iter()
                            .enumerate()
                            .map(|(i, c)| self.chara_item(i, c, cx)),
                    ),
            )
            .child(self.chara_list_actions(cx))
            .into_any_element()
    }

    fn chara_item(&self, i: usize, c: &Chara, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.status.id.is_none();

        let id = c.id.clone();
        let selected = self.state.snapshot.editor.active_chara == id;
        div()
            .id(("chara", i))
            .min_h(px(44.))
            .mb_1()
            .rounded(px(8.))
            .flex()
            .items_center()
            .gap_2()
            .px(px(8.))
            .when(selected, |el| el.bg(p.control))
            .text_color(if selected { p.ink } else { p.muted })
            .when(!disabled, |el| el.cursor_pointer().hover(|s| s.bg(p.hover)))
            .child(self.avatar(Some(c), 32.))
            .child(div().text_xs().overflow_hidden().child(c.name.clone()))
            .when(!disabled, |el| {
                el.on_click(cx.listener(move |this, _, w, cx| {
                    if this.state.select_chara(id.clone()) {
                        this.edited(w, cx);
                    }
                }))
            })
            .into_any_element()
    }

    fn chara_list_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let disabled = self.state.busy || self.state.status.id.is_none();
        let active = self
            .state
            .snapshot
            .charas
            .iter()
            .find(|c| c.id == self.state.snapshot.editor.active_chara)
            .cloned();
        let no_active = active.is_none();
        div()
            .flex()
            .gap_1()
            .px_2()
            .py_1()
            .border_t_1()
            .border_color(p.line)
            .child(
                icon_button("add-chara", "plus")
                    .size(px(32.))
                    .disabled_with_cursor(disabled)
                    .on_click(cx.listener(|this, _, _, cx| this.create_chara(cx))),
            )
            .child(
                icon_button("duplicate-chara", "copy")
                    .size(px(32.))
                    .disabled_with_cursor(disabled || no_active)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(command) = this.state.duplicate_chara() {
                            this.send(command);
                            cx.notify();
                        }
                    })),
            )
            .child(
                icon_button(
                    "lock-chara",
                    if self.state.chara_locked {
                        "lock"
                    } else {
                        "unlock"
                    },
                )
                .size(px(32.))
                .when(self.state.chara_locked, |el| {
                    el.bg(p.soft).text_color(self.accent())
                })
                .disabled_with_cursor(disabled || no_active)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.state.chara_locked = !this.state.chara_locked;
                    this.sync_fields(window, cx);
                    cx.notify();
                })),
            )
            .child(
                icon_button("delete-chara", "trash")
                    .size(px(32.))
                    .bg(rgb(0xb52e42))
                    .text_color(rgb(0xffffff))
                    .disabled_with_cursor(disabled || no_active || self.state.chara_locked)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(chara) = &active {
                            this.delete_chara(chara.id.clone(), chara.name.clone(), window, cx);
                        }
                    })),
            )
            .into_any_element()
    }
}
