//! 关于浮层：显示版本与当前构建说明

use gpui::{AnyElement, Context, Window, div, prelude::*, px};

use super::Spike;
use super::theme::RADIUS_ITEM;
use super::widgets::{overlay, overlay_panel, panel_header};
use crate::text::TextKey;

#[derive(Clone, Copy)]
pub(super) enum InfoKind {
    About,
}

impl Spike {
    pub(super) fn open_info(&mut self, kind: InfoKind, cx: &mut Context<Self>) {
        self.open_menu = None;
        self.menu_selected = None;
        if std::mem::take(&mut self.settings_open) {
            self.commit_opacity_quiet(cx);
        }
        self.help_open = false;
        self.photo_info = None;
        self.info = Some(kind);
        cx.notify();
    }

    pub(super) fn close_info(&mut self, cx: &mut Context<Self>) {
        if self.info.take().is_some() {
            cx.notify();
        }
    }

    pub(super) fn render_info_panel(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.info?;
        let palette = self.palette(window);
        let t = self.text();
        let title = t.t(TextKey::About);
        let lines = vec![
            format!(
                "{} {}",
                t.t(TextKey::AboutVersion),
                env!("CARGO_PKG_VERSION")
            ),
            t.t(TextKey::AboutStage).to_string(),
            t.t(TextKey::AboutFeatures).to_string(),
        ];

        let mut body = div().flex().flex_col().gap(px(8.0));
        for line in lines {
            body = body.child(
                div()
                    .text_size(px(12.0))
                    .text_color(palette.text_muted)
                    .child(line),
            );
        }

        let panel = overlay_panel("info-panel", 360.0, palette, window.viewport_size())
            .shadow_md()
            .child(panel_header(title, t.t(TextKey::EscClose), palette))
            .child(body)
            .child(
                div().flex().justify_end().child(
                    div()
                        .id("info-done")
                        .h(px(28.0))
                        .px(px(16.0))
                        .flex()
                        .items_center()
                        .rounded(px(RADIUS_ITEM + 2.0))
                        .bg(palette.accent)
                        .text_size(px(12.0))
                        .text_color(palette.on_accent)
                        .cursor_pointer()
                        .hover(|style| style.opacity(0.9))
                        .on_click(cx.listener(|this, _, _, cx| this.close_info(cx)))
                        .child(t.t(TextKey::GotIt)),
                ),
            );

        Some(overlay(panel.into_any_element(), cx, |this, _, cx| {
            this.close_info(cx);
        }))
    }
}
