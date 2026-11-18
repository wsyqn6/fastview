//! 下拉菜单浮层与菜单内交互
//!
//! 面板放在窗口内而不是单独的 popup 窗口：复用同一个 renderer / atlas，
//! 也省掉定位、失焦、多显示器翻转那一整套系统语义。

use gpui::{
    AnyElement, BoxShadow, Context, MouseButton, MouseDownEvent, MouseMoveEvent, Point, Pixels,
    Window, div, hsla, prelude::*, px,
};

use super::Spike;
use super::theme::{MENU_WIDTH, Palette, TITLEBAR_HEIGHT};
use super::widgets::mnemonic_label;
use crate::menu;

impl Spike {
    /// Alt+助记符：切换顶层菜单；未命中则收起已展开的菜单（同原生行为）
    pub(super) fn press_mnemonic(&mut self, key: char, window: &mut Window, cx: &mut Context<Self>) {
        match menu::menu_by_mnemonic(key) {
            Some(index) => self.press_menu(index, window, cx),
            None => {
                if self.open_menu.take().is_some() {
                    self.menu_selected = None;
                    cx.notify();
                }
            }
        }
    }

    /// 点击 / 再次点击顶层菜单：展开、收起，或执行顶层命令按钮
    pub(super) fn press_menu(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(spec) = menu::MENUS.get(index) else {
            return;
        };

        // 顶层命令按钮（无下拉）直接执行
        if spec.items.is_empty() {
            self.open_menu = None;
            self.dispatch(menu::Cmd::Settings, window, cx);
            return;
        }

        self.open_menu = if self.open_menu == Some(index) {
            None
        } else {
            Some(index)
        };
        self.menu_selected = None;
        // 让 Esc 等按键事件仍然送到本窗口
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    /// 收起下拉（不触发重绘，调用方决定何时 `notify`）
    pub(super) fn close_menu(&mut self) -> bool {
        self.menu_selected = None;
        self.open_menu.take().is_some()
    }

    /// 窗口坐标落在哪个顶层菜单上
    fn menu_at(&self, position: Point<Pixels>) -> Option<usize> {
        let rects = self.menu_rects.borrow();
        menu::MENUS.iter().enumerate().find_map(|(index, _)| {
            let rect = rects.get(index)?;
            rect.contains(&position).then_some(index)
        })
    }

    /// 菜单展开时，鼠标滑到别的顶层菜单就跟着切换（同系统菜单行为）
    fn hover_menu_switch(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(index) = self.menu_at(position) else {
            return;
        };
        if self.open_menu == Some(index) || menu::MENUS[index].items.is_empty() {
            return;
        }
        self.open_menu = Some(index);
        self.menu_selected = None;
        cx.notify();
    }

    /// 上下键在可执行条目之间移动（跳过分隔线）
    pub(super) fn move_menu_selection(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(spec) = self.open_menu.and_then(|index| menu::MENUS.get(index)) else {
            return;
        };
        let commands: Vec<usize> = spec
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| matches!(item, menu::MenuItem::Command { .. }))
            .map(|(index, _)| index)
            .collect();
        let Some(last) = commands.len().checked_sub(1) else {
            return;
        };

        let next = match self
            .menu_selected
            .and_then(|selected| commands.iter().position(|index| *index == selected))
        {
            Some(position) if forward => commands[(position + 1) % commands.len()],
            Some(position) => commands[(position + last) % commands.len()],
            None if forward => commands[0],
            None => commands[last],
        };
        self.menu_selected = Some(next);
        cx.notify();
    }

    /// 左右键切换顶层菜单（跳过纯命令按钮）
    pub(super) fn switch_menu(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(current) = self.open_menu else {
            return;
        };
        let count = menu::MENUS.len();
        for step in 1..=count {
            let index = if forward {
                (current + step) % count
            } else {
                (current + count - step) % count
            };
            if !menu::MENUS[index].items.is_empty() {
                self.open_menu = Some(index);
                self.menu_selected = None;
                cx.notify();
                return;
            }
        }
    }

    /// 当前键盘选中的命令
    pub(super) fn selected_cmd(&self) -> Option<menu::Cmd> {
        let spec = self.open_menu.and_then(|index| menu::MENUS.get(index))?;
        match spec.items.get(self.menu_selected?) {
            Some(menu::MenuItem::Command { cmd, .. }) => Some(*cmd),
            _ => None,
        }
    }

    /// 下拉菜单浮层：全窗遮罩负责关闭与切换，面板自绘
    pub(super) fn render_menu_layer(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let index = self.open_menu?;
        let spec = menu::MENUS.get(index)?;
        let palette = Palette::for_appearance(window.appearance());

        let window_width: f32 = window.viewport_size().width.into();
        let item_x = self
            .menu_rects
            .borrow()
            .get(index)
            .map(|rect| f32::from(rect.origin.x))
            .unwrap_or(8.0);
        // 贴右边缘时往左收，保证面板完整落在窗口里
        let panel_x = item_x.min((window_width - MENU_WIDTH - 8.0).max(8.0));

        let backdrop = div()
            .absolute()
            .inset_0()
            // 菜单打开期间挡住标题栏与内容区，避免误触窗口拖拽/查看器
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    // 点在顶层菜单上：切换/执行；点在别处：收起
                    if let Some(index) = this.menu_at(event.position) {
                        this.press_menu(index, window, cx);
                    } else if this.close_menu() {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                this.hover_menu_switch(event.position, cx);
            }))
            .on_scroll(cx.listener(|this, _, _, cx| {
                if this.close_menu() {
                    cx.notify();
                }
            }));

        let mut panel = div()
            .absolute()
            .left(px(panel_x))
            .top(px(TITLEBAR_HEIGHT))
            .w(px(MENU_WIDTH))
            .flex()
            .flex_col()
            .p(px(4.0))
            .rounded(px(8.0))
            .bg(palette.menu_bg)
            .border_1()
            .border_color(palette.menu_border)
            .shadow(vec![
                BoxShadow::new(px(0.0), px(6.0), hsla(0.0, 0.0, 0.0, 0.22))
                    .blur_radius(px(18.0))
                    .spread_radius(px(-6.0)),
                BoxShadow::new(px(0.0), px(1.0), hsla(0.0, 0.0, 0.0, 0.14)).blur_radius(px(3.0)),
            ])
            // 面板空白处也不要漏到遮罩上（否则点分隔线会关掉菜单）
            .occlude();

        for (item_index, item) in spec.items.iter().enumerate() {
            match item {
                menu::MenuItem::Separator => {
                    panel = panel.child(
                        div()
                            .h(px(1.0))
                            .mx(px(8.0))
                            .my(px(4.0))
                            .bg(palette.menu_border),
                    );
                }
                menu::MenuItem::Command {
                    label,
                    shortcut,
                    mnemonic,
                    cmd,
                } => {
                    let cmd = *cmd;
                    let selected = self.menu_selected == Some(item_index);
                    let mut row = div()
                        .id(("menu-row", item_index))
                        .h(px(28.0))
                        .px(px(8.0))
                        .flex()
                        .items_center()
                        .rounded(px(4.0))
                        .text_size(px(12.0))
                        .text_color(palette.text)
                        .when(selected, |this| this.bg(palette.active))
                        .hover(move |style| style.bg(palette.hover))
                        // 悬停同步选中项：此时按回车执行的就是鼠标指向的命令
                        .on_mouse_move(cx.listener(move |this, _, _, cx| {
                            if this.menu_selected != Some(item_index) {
                                this.menu_selected = Some(item_index);
                                cx.notify();
                            }
                        }))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.dispatch(cmd, window, cx);
                        }))
                        // 菜单展开时条目助记符始终可见（同系统菜单）
                        .child(mnemonic_label(label, *mnemonic, palette.text, true));

                    if !shortcut.is_empty() {
                        row = row.child(div().flex_1()).child(
                            div()
                                .text_size(px(11.0))
                                .text_color(palette.text_muted)
                                .child(*shortcut),
                        );
                    }
                    panel = panel.child(row);
                }
            }
        }

        Some(
            div()
                .absolute()
                .inset_0()
                .child(backdrop)
                .child(panel)
                .into_any_element(),
        )
    }
}
