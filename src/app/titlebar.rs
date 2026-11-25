//! 自绘标题栏：左边菜单、右边窗口按钮，空白处可拖拽 / 双击最大化

use gpui::{AnyElement, Context, Div, MouseButton, Window, WindowControlArea, div, prelude::*, px};

use super::Spike;
use super::theme::{Palette, TITLEBAR_HEIGHT, WINDOW_BUTTON_WIDTH};
use super::widgets::{Icon, app_logo, mnemonic_label, window_button};
use crate::menu;

impl Spike {
    pub(super) fn render_titlebar(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let is_maximized = window.is_maximized();
        let palette = self.palette(window);
        let t = self.text();
        // 助记符下划线：按住 Alt 或菜单展开时显示
        let mnemonics = self.alt_held || self.open_menu.is_some();

        // 标题浮层相对整条栏居中；预留两侧控件宽度，长文件名只在中间截断
        let side_reserve = {
            let rects = self.menu_rects.borrow();
            let left = rects
                .last()
                .map(|r| f32::from(r.origin.x) + f32::from(r.size.width))
                .unwrap_or(8.0 + menu::MENUS.len() as f32 * 52.0);
            left.max(3.0 * WINDOW_BUTTON_WIDTH)
        };
        let title_max =
            (f32::from(window.viewport_size().width) - 2.0 * side_reserve - 32.0).max(0.0);
        let title = self
            .image
            .as_ref()
            .map(|image| image.file_name())
            .unwrap_or_default();

        let bar = div()
            .flex()
            .flex_row()
            .items_center()
            .relative()
            .h(px(TITLEBAR_HEIGHT))
            // 半透明标题栏：与主体只靠这点 alpha 差区分，不加分隔线
            .bg(palette.titlebar)
            .child(title_layer(title, title_max, palette))
            // 左边缘留白也可拖动（需要显式 id 才会生成命中盒）
            .child(
                div()
                    .id("titlebar-drag-left")
                    .w(px(8.0))
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            )
            // 应用标与左留白一样交给系统拖动，不新增点击行为
            .child(
                div()
                    .id("titlebar-app-logo")
                    .h_full()
                    .flex()
                    .items_center()
                    .window_control_area(WindowControlArea::Drag)
                    .child(app_logo(palette.icon)),
            )
            .child(self.menu_strip(t, mnemonics, palette, cx))
            // 空白区即拖拽区：拖动分屏、双击最大化都交给系统（HTCAPTION）
            .child(
                div()
                    .id("titlebar-drag")
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            );

        bar.child(window_button(
            "win-min",
            Icon::Minimize,
            false,
            WindowControlArea::Min,
            palette,
        ))
        .child(window_button(
            "win-max",
            if is_maximized {
                Icon::Restore
            } else {
                Icon::Maximize
            },
            false,
            WindowControlArea::Max,
            palette,
        ))
        .child(window_button(
            "win-close",
            Icon::Close,
            true,
            WindowControlArea::Close,
            palette,
        ))
        .into_any_element()
    }

    /// 菜单项单独包一层：`on_children_prepainted` 的索引与 `MENUS` 一一对应
    fn menu_strip(
        &self,
        t: crate::text::Text,
        mnemonics: bool,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut menus = div().flex().flex_row().items_center().pl(px(8.0));
        for (index, spec) in menu::MENUS.iter().enumerate() {
            let open = self.open_menu == Some(index);
            menus = menus.child(
                div()
                    .id(("top-menu", index))
                    .h(px(26.0))
                    .px(px(10.0))
                    .flex()
                    .items_center()
                    .rounded(px(4.0))
                    .text_size(px(12.0))
                    .text_color(palette.text)
                    .when(open, |this| this.bg(palette.active))
                    .when(!open, |this| {
                        this.hover(move |style| style.bg(palette.hover))
                            .active(move |style| style.bg(palette.active))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.press_menu(index, window, cx);
                        }),
                    )
                    .child(mnemonic_label(
                        t.t(spec.title),
                        spec.mnemonic,
                        palette.text,
                        mnemonics,
                    )),
            );
        }
        menus.on_children_prepainted({
            let rects = self.menu_rects.clone();
            move |bounds, _, _| *rects.borrow_mut() = bounds
        })
    }
}

/// 标题浮层置于控件下层；只有文字盒带 Drag 命中，不挡菜单与窗口按钮
fn title_layer(title: String, title_max: f32, palette: Palette) -> Div {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .id("titlebar-title")
                .px(px(16.0))
                .max_w(px(title_max))
                .text_size(px(12.0))
                .truncate()
                .text_color(palette.text_muted)
                .window_control_area(WindowControlArea::Drag)
                .child(title),
        )
}
