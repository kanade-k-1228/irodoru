use super::Studio;
use super::app_menu::MenuAction;
use super::appearance::{button, icon, tool};
use gpui_kit::component::Disableable;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Render for Studio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette();
        let _ = &self.subscriptions;
        div()
            .relative()
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &super::actions::NewWindow, w, cx| {
                this.menu_select(MenuAction::NewWindow, w, cx)
            }))
            .on_action(cx.listener(|this, _: &super::actions::NewProject, w, cx| {
                this.menu_select(MenuAction::NewProject, w, cx)
            }))
            .on_action(cx.listener(|this, _: &super::actions::OpenProject, w, cx| {
                this.menu_select(MenuAction::OpenProject, w, cx)
            }))
            .on_action(cx.listener(|this, _: &super::actions::SaveProject, w, cx| {
                this.menu_select(MenuAction::SaveProject, w, cx)
            }))
            .on_action(cx.listener(|this, _: &super::actions::Preferences, w, cx| {
                this.menu_select(MenuAction::Preferences, w, cx)
            }))
            .on_action(cx.listener(|this, _: &super::actions::CloseWindow, w, cx| {
                this.menu_select(MenuAction::CloseWindow, w, cx)
            }))
            .on_action(cx.listener(|this, _: &super::actions::Quit, w, cx| {
                this.menu_select(MenuAction::CloseWindow, w, cx)
            }))
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(p.surface)
            .text_color(p.ink)
            .text_sm()
            .when(!cfg!(target_os = "macos"), |el| {
                el.child(self.header(window, cx))
            })
            .when_some(self.state.error.clone(), |el, error| {
                el.child(self.error_banner(error, cx))
            })
            .child(self.script(cx))
            .child(self.character_panel(window, cx))
            .child(self.footer(cx))
            .when(self.menu_open, |el| el.child(self.app_menu(cx)))
    }
}

impl Studio {
    fn error_banner(&self, error: String, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
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
            .child(
                tool("dismiss-error", "close", "エラーを閉じる").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.state.error = None;
                        cx.notify();
                    },
                )),
            )
            .into_any_element()
    }

    fn character_panel(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let narrow = window.viewport_size().width < px(640.);
        div()
            .flex()
            .h(px(if self.state.draft.is_some() {
                (f32::from(window.viewport_size().height) * 0.5).min(360.)
            } else {
                136.
            }))
            .flex_shrink_0()
            .min_h_0()
            .border_t_1()
            .border_color(p.line)
            .bg(p.panel)
            .child(self.characters(narrow, cx))
            .child(if self.state.draft.is_some() {
                self.character_editor(narrow, cx)
            } else {
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        button("create-character", "キャラクターを作成")
                            .icon(icon("plus"))
                            .disabled(self.state.busy || self.state.status.id.is_none())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.edit_character(None, window, cx);
                            })),
                    )
                    .into_any_element()
            })
            .into_any_element()
    }

    fn footer(&self, cx: &mut Context<Self>) -> AnyElement {
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
