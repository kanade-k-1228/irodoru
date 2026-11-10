use super::appearance::{button, icon};
use super::modal::MenuAction;
use crate::domain::InferenceStatus;
use crate::workspace::view::Studio;
use gpui_kit::component::Disableable;
use gpui_kit::component::button::{Button, ButtonVariants};
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
                    // Center the single line without a nested flex container.
                    .line_height(px(32.))
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
                self.footer_button("footer-export", "download", "出力")
                    .tooltip("セリフごとにWAVを書き出す")
                    .disabled(
                        self.state.busy
                            || self.state.status.id.is_none()
                            || self.state.snapshot.editor.lines.is_empty(),
                    )
                    .dropdown_menu_with_anchor(Anchor::BottomRight, move |mut menu, _, _| {
                        for naming in crate::project::ExportNaming::ALL {
                            let studio = studio.clone();
                            menu = menu.item(PopupMenuItem::new(naming.label()).on_click(
                                move |_, window, cx| {
                                    let _ = studio.update(cx, |this, cx| {
                                        this.export_lines(naming, window, cx)
                                    });
                                },
                            ));
                        }
                        menu
                    }),
            )
            .child(
                self.footer_button(
                    "footer-settings",
                    MenuAction::Preferences.icon(),
                    MenuAction::Preferences.label(),
                )
                .track_focus(&self.menu_trigger_focus)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.menu_select(MenuAction::Preferences, window, cx);
                })),
            )
            .into_any_element()
    }

    fn model_status_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let status = self.model_status();
        let failed = matches!(status, InferenceStatus::Error { .. });
        let providers = self
            .inference
            .as_ref()
            .map(|service| service.providers())
            .unwrap_or_default();
        let mut names: Vec<_> = providers.iter().map(|p| p.provider.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        let status_label = super::modal::status_label(&status);
        let label = if names.is_empty() {
            status_label
        } else {
            format!("{status_label} · {}", names.join(" + "))
        };
        self.footer_button("model-settings", "model", label)
            .track_focus(&self.model_trigger_focus)
            .text_color(if failed { p.warning } else { p.muted })
            .tooltip("モデル設定")
            .on_click(cx.listener(|this, _, window, cx| this.show_model_settings(window, cx)))
            .into_any_element()
    }

    /// An icon with its label, separated from the item on its left.
    pub(super) fn footer_button(
        &self,
        id: &'static str,
        icon_name: &str,
        label: impl Into<SharedString>,
    ) -> Button {
        button(id, "")
            .ghost()
            .flex_shrink_0()
            .h(px(32.))
            .rounded_none()
            .border_l_1()
            .border_color(self.palette().line)
            .icon(icon(icon_name))
            .label(label)
    }
}
