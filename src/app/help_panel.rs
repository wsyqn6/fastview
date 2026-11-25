//! 快捷键说明浮层
//!
//! 与设置浮层同一套做法：全窗遮罩负责关闭，面板自绘。
//! 只列当前真实可用的快捷键，避免说明与实现脱节。

use gpui::{AnyElement, BoxShadow, Context, Div, Window, div, prelude::*, px};

use super::Spike;
use super::theme::{Palette, RADIUS_ITEM};
use super::widgets::{overlay, overlay_panel, panel_header};
use crate::text::{Text, TextKey};

/// 面板宽度：两列分组
const PANEL_WIDTH: f32 = 460.0;

/// 按键标签：字面量无需翻译，中文描述走文案表
#[derive(Clone, Copy)]
enum KeyLabel<'a> {
    Raw(&'a str),
    I18n(TextKey),
}

/// 左列：文件 / 查看
const LEFT: &[(TextKey, &[(TextKey, KeyLabel)])] = &[
    (
        TextKey::GroupFile,
        &[
            (TextKey::ActOpen, KeyLabel::Raw("Ctrl+O")),
            (TextKey::ActSettings, KeyLabel::Raw("Ctrl+,")),
        ],
    ),
    (
        TextKey::GroupView,
        &[
            (TextKey::ActFit, KeyLabel::Raw("0")),
            (TextKey::ActOriginal, KeyLabel::Raw("1")),
            (TextKey::ActFill, KeyLabel::Raw("2")),
            (TextKey::ActRotateCw, KeyLabel::Raw("R")),
            (TextKey::ActRotateCcw, KeyLabel::Raw("Shift+R")),
            (TextKey::ActZoom, KeyLabel::Raw("+ / -")),
            (TextKey::ActZoomCursor, KeyLabel::I18n(TextKey::KeyWheel)),
            (TextKey::ActStep, KeyLabel::Raw("← / →")),
            (TextKey::ActPan, KeyLabel::I18n(TextKey::KeyDrag)),
            (TextKey::ActThumbnails, KeyLabel::Raw("T")),
            (TextKey::ActStatus, KeyLabel::Raw("S")),
            (TextKey::ActPhotoInfo, KeyLabel::Raw("I")),
        ],
    ),
];

/// 右列：窗口 / 帮助
const RIGHT: &[(TextKey, &[(TextKey, KeyLabel)])] = &[
    (
        TextKey::GroupWindow,
        &[
            (TextKey::ActFullscreen, KeyLabel::Raw("F / F11")),
            (TextKey::ActBorderless, KeyLabel::Raw("V")),
            (TextKey::ActExitFullscreen, KeyLabel::Raw("Esc")),
        ],
    ),
    (
        TextKey::GroupHelp,
        &[(TextKey::ActShortcuts, KeyLabel::Raw("H / ?"))],
    ),
];

impl Spike {
    pub(super) fn open_help(&mut self, cx: &mut Context<Self>) {
        if self.help_open {
            return;
        }
        // 两个浮层互斥，避免叠在一起；设置改动照常落盘
        self.open_menu = None;
        self.menu_selected = None;
        if std::mem::take(&mut self.settings_open) {
            self.commit_opacity_quiet(cx);
        }
        self.info = None;
        self.photo_info = None;
        self.help_open = true;
        cx.notify();
    }

    pub(super) fn close_help(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.help_open) {
            cx.notify();
        }
    }

    pub(super) fn toggle_help(&mut self, cx: &mut Context<Self>) {
        if self.help_open {
            self.close_help(cx);
        } else {
            self.open_help(cx);
        }
    }

    pub(super) fn render_help_panel(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.help_open {
            return None;
        }
        let palette = self.palette(window);
        let t = self.text();

        let panel = overlay_panel("help-panel", PANEL_WIDTH, palette, window.viewport_size())
            .shadow(vec![
                BoxShadow::new(px(0.0), px(8.0), palette.shadow_strong)
                    .blur_radius(px(24.0))
                    .spread_radius(px(-6.0)),
                BoxShadow::new(px(0.0), px(1.0), palette.shadow_soft).blur_radius(px(3.0)),
            ])
            .child(panel_header(
                t.t(TextKey::ShortcutsTitle),
                t.t(TextKey::ShortcutsClose),
                palette,
            ))
            .child(
                div()
                    .flex()
                    .gap(px(28.0))
                    .child(column(LEFT, t, &palette))
                    .child(column(RIGHT, t, &palette)),
            );

        Some(overlay(panel.into_any_element(), cx, |this, _, cx| {
            this.close_help(cx);
        }))
    }
}

/// 一列分组
fn column(groups: &[(TextKey, &[(TextKey, KeyLabel)])], t: Text, palette: &Palette) -> Div {
    let mut column = div().flex().flex_col().gap(px(14.0)).flex_1();
    for (title, rows) in groups {
        column = column.child(group(*title, rows, t, palette));
    }
    column
}

/// 一个分组：标题 + 若干「动作 / 按键」
fn group(title: TextKey, rows: &[(TextKey, KeyLabel)], t: Text, palette: &Palette) -> Div {
    let mut group = div().flex().flex_col().gap(px(2.0)).child(
        div()
            .mb(px(4.0))
            .text_size(px(11.0))
            .text_color(palette.text_muted)
            .child(t.t(title)),
    );
    for (action, keys) in rows {
        group = group.child(row(*action, *keys, t, palette));
    }
    group
}

fn row(action: TextKey, keys: KeyLabel, t: Text, palette: &Palette) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .h(px(26.0))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(palette.text)
                .child(t.t(action)),
        )
        .child(
            div()
                .px(px(6.0))
                .py(px(2.0))
                .rounded(px(RADIUS_ITEM))
                .bg(palette.hover)
                .text_size(px(11.0))
                .text_color(palette.text_muted)
                .child(match keys {
                    KeyLabel::Raw(label) => label.to_string(),
                    KeyLabel::I18n(key) => t.t(key).to_string(),
                }),
        )
}
