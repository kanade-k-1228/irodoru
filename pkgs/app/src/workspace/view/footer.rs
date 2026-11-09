use super::app_menu::MenuAction;
use super::appearance::tool;
use crate::workspace::view::Studio;
use gpui_kit::component::Disableable;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(in crate::workspace::view) fn footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let studio = cx.entity().downgrade();
        div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .border_t_1()
            .border_color(p.line)
            .bg(p.panel)
            .h(px(32.))
            .text_xs()
            .line_height(px(16.))
            .text_color(p.muted)
            .child(
                div()
                    .id("project-menu-trigger")
                    .track_focus(&self.file_trigger_focus)
                    .flex_1()
                    .min_w_0()
                    .cursor_pointer()
                    .h_full()
                    .px_2()
                    .flex()
                    .items_center()
                    .hover(|style| style.bg(p.hover))
                    .overflow_hidden()
                    .text_ellipsis()
                    .when(self.state.status.path.is_none(), |el| {
                        el.text_color(p.warning)
                    })
                    .child(
                        self.state
                            .status
                            .path
                            .clone()
                            .unwrap_or_else(|| "プロジェクト未選択".into()),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_project_menu(window, cx);
                    })),
            )
            .child(self.model_status_button(cx))
            .child(
                tool("footer-export", "download", "セリフごとにWAVを書き出す")
                    .w(px(32.))
                    .h_full()
                    .rounded_none()
                    .border_l_1()
                    .border_color(p.line)
                    .flex_shrink_0()
                    .disabled(
                        self.state.busy
                            || self.state.draft.is_some()
                            || self.state.status.id.is_none()
                            || self.state.snapshot.editor.rows.is_empty(),
                    )
                    .dropdown_menu_with_anchor(Anchor::BottomRight, move |mut menu, _, _| {
                        for naming in crate::project::ExportNaming::ALL {
                            let studio = studio.clone();
                            menu = menu.item(PopupMenuItem::new(naming.label()).on_click(
                                move |_, window, cx| {
                                    let _ = studio.update(cx, |this, cx| {
                                        this.export_script(naming, window, cx)
                                    });
                                },
                            ));
                        }
                        menu
                    }),
            )
            .child(
                tool("footer-settings", "gear", "設定")
                    .track_focus(&self.menu_trigger_focus)
                    .w(px(32.))
                    .h_full()
                    .rounded_none()
                    .border_l_1()
                    .border_color(p.line)
                    .flex_shrink_0()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.menu_select(MenuAction::Preferences, window, cx);
                    })),
            )
            .into_any_element()
    }
}
