use super::Modal;
use crate::project::ExportNaming;
use crate::workspace::view::Studio;
use crate::workspace::view::appearance::{blend, button};
use gpui_kit::base::Radio;
use gpui_kit::component::button::ButtonVariants;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

impl Studio {
    pub(in crate::workspace::view) fn show_export(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_modal(
            Modal::Export,
            560.,
            || "出力".into_any_element(),
            |this, cx| {
                div()
                    .mt_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(this.palette().muted)
                            .child("命名規則"),
                    )
                    .children(
                        ExportNaming::ALL
                            .into_iter()
                            .enumerate()
                            .map(|(i, naming)| this.export_naming_item(i, naming, cx)),
                    )
                    .child(
                        button("export-confirm", "エクスポート")
                            .primary()
                            .w_full()
                            .mt_3()
                            .on_click(cx.listener(|this, _, window, cx| {
                                let naming = this.export_naming;
                                this.close_modal(window, cx);
                                this.export_lines(naming, window, cx);
                            })),
                    )
                    .into_any_element()
            },
            window,
            cx,
        );
    }

    fn export_naming_item(
        &self,
        i: usize,
        naming: ExportNaming,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette();
        let selected = self.export_naming == naming;
        let select = cx.listener(move |this, _: &bool, window, cx| {
            this.export_naming = naming;
            window.refresh();
            cx.notify();
        });
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                Radio::new(("export-naming", i))
                    .accessibility_label(naming.label())
                    .size(px(22.))
                    .flex_shrink_0()
                    .rounded_full()
                    .border_2()
                    .border_color(if selected { self.accent() } else { p.muted })
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .when(selected, |el| {
                        el.child(div().size(px(10.)).rounded_full().bg(self.accent()))
                    })
                    .checked(selected)
                    .on_change(move |checked, _, window, cx| select(&checked, window, cx)),
            )
            .child(
                div()
                    .id(("export-naming-label", i))
                    .flex_1()
                    .min_w_0()
                    .border_1()
                    .border_color(if selected { self.accent() } else { p.line })
                    .bg(if selected {
                        blend(self.accent(), p.panel, 0.12)
                    } else {
                        p.panel
                    })
                    .rounded(px(8.))
                    .px_2()
                    .py_2()
                    .text_sm()
                    .cursor_pointer()
                    .child(naming.label())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.export_naming = naming;
                        window.refresh();
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
