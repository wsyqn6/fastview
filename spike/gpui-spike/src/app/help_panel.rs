//! 快捷键说明浮层
//!
//! 与设置浮层同一套做法：全窗遮罩负责关闭，面板自绘。
//! 只列当前真实可用的快捷键，避免说明与实现脱节。

use gpui::{
    AnyElement, BoxShadow, Context, Div, MouseButton, MouseDownEvent, Window, div, hsla,
    prelude::*, px,
};

use super::Spike;
use super::theme::{Palette, RADIUS_ITEM, RADIUS_PANEL};

/// 面板宽度：两列分组
const PANEL_WIDTH: f32 = 460.0;

/// 左列：文件 / 查看
const LEFT: &[(&str, &[(&str, &str)])] = &[
    ("文件", &[("打开文件", "Ctrl+O"), ("设置", "Ctrl+,")]),
    (
        "查看",
        &[
            ("适应窗口", "0"),
            ("原始大小", "1"),
            ("填充窗口", "2"),
            ("顺时针旋转", "R"),
            ("逆时针旋转", "Shift+R"),
            ("放大 / 缩小", "+ / -"),
            ("以光标缩放", "滚轮"),
            ("上一张 / 下一张", "← / →"),
            ("平移", "拖拽"),
            ("缩略图", "T"),
            ("状态栏常显", "S"),
        ],
    ),
];

/// 右列：窗口 / 帮助
const RIGHT: &[(&str, &[(&str, &str)])] = &[
    (
        "窗口",
        &[
            ("全屏", "F / F11"),
            ("无边框", "V"),
            ("退出全屏 / 关闭浮层", "Esc"),
        ],
    ),
    ("帮助", &[("快捷键", "H / ?")]),
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
            self.settings_drag = false;
            self.settings.save();
        }
        self.info = None;
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

        let backdrop = div()
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| this.close_help(cx)),
            );

        let panel = div()
            .id("help-panel")
            .w(px(PANEL_WIDTH))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .p(px(20.0))
            .rounded(px(RADIUS_PANEL + 2.0))
            .bg(palette.menu_bg)
            .border_1()
            .border_color(palette.menu_border)
            .shadow(vec![
                BoxShadow::new(px(0.0), px(8.0), hsla(0.0, 0.0, 0.0, 0.24))
                    .blur_radius(px(24.0))
                    .spread_radius(px(-6.0)),
                BoxShadow::new(px(0.0), px(1.0), hsla(0.0, 0.0, 0.0, 0.14)).blur_radius(px(3.0)),
            ])
            // 面板内空白处不把点击漏给遮罩
            .occlude()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(palette.text)
                            .child("快捷键"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(palette.text_muted)
                            .child("H / Esc 关闭"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(28.0))
                    .child(column(LEFT, &palette))
                    .child(column(RIGHT, &palette)),
            );

        Some(
            div()
                .absolute()
                .inset_0()
                .child(backdrop)
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(panel),
                )
                .into_any_element(),
        )
    }
}

/// 一列分组
fn column(groups: &[(&str, &[(&str, &str)])], palette: &Palette) -> Div {
    let mut column = div().flex().flex_col().gap(px(14.0)).flex_1();
    for (title, rows) in groups {
        column = column.child(group(title, rows, palette));
    }
    column
}

/// 一个分组：标题 + 若干「动作 / 按键」
fn group(title: &str, rows: &[(&str, &str)], palette: &Palette) -> Div {
    let mut group = div().flex().flex_col().gap(px(2.0)).child(
        div()
            .mb(px(4.0))
            .text_size(px(11.0))
            .text_color(palette.text_muted)
            .child(title.to_string()),
    );
    for (action, keys) in rows {
        group = group.child(row(action, keys, palette));
    }
    group
}

fn row(action: &str, keys: &str, palette: &Palette) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .h(px(26.0))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(palette.text)
                .child(action.to_string()),
        )
        .child(
            div()
                .px(px(6.0))
                .py(px(2.0))
                .rounded(px(RADIUS_ITEM))
                .bg(palette.hover)
                .text_size(px(11.0))
                .text_color(palette.text_muted)
                .child(keys.to_string()),
        )
}
