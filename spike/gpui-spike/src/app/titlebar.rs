//! 自绘标题栏：左边菜单、右边窗口按钮，空白处可拖拽 / 双击最大化

use gpui::{
    AnyElement, Context, MouseButton, Window, WindowControlArea, div, px, prelude::*,
};

use super::Spike;
use super::theme::TITLEBAR_HEIGHT;
use super::widgets::{Icon, mnemonic_label, window_button};
use crate::menu;

impl Spike {
    pub(super) fn render_titlebar(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let is_maximized = window.is_maximized();
        let palette = self.palette(window);
        // 助记符下划线：按住 Alt 或菜单展开时显示
        let mnemonics = self.alt_held || self.open_menu.is_some();

        let mut bar = div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(TITLEBAR_HEIGHT))
            // 半透明标题栏：与主体只靠这点 alpha 差区分，不加分隔线
            .bg(palette.titlebar)
            // 左边缘留白也可拖动（需要显式 id 才会生成命中盒）
            .child(
                div()
                    .id("titlebar-drag-left")
                    .w(px(8.0))
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            );

        // 菜单项单独包一层：`on_children_prepainted` 的索引与 `MENUS` 一一对应，
        // 不受标题栏其它子元素（留白、窗口按钮）的数量与顺序影响
        let mut menus = div().flex().flex_row().items_center();
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
                    .child(mnemonic_label(spec.title, spec.mnemonic, palette.text, mnemonics)),
            );
        }
        bar = bar.child(menus.on_children_prepainted({
            let rects = self.menu_rects.clone();
            move |bounds, _, _| *rects.borrow_mut() = bounds
        }));

        // 空白区即拖拽区：拖动分屏、双击最大化都交给系统（HTCAPTION）
        bar.child(
            div()
                .id("titlebar-drag")
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .window_control_area(WindowControlArea::Drag)
                .child(
                    div()
                        .px(px(16.0))
                        .text_size(px(12.0))
                        .truncate()
                        .text_color(palette.text_muted)
                        .child(
                            self.image
                                .as_ref()
                                .map(|image| image.file_name())
                                .unwrap_or_default(),
                        ),
                ),
        )
        .child(window_button(
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
}
