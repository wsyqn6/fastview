//! 下拉菜单浮层与菜单内交互
//!
//! 面板放在窗口内而不是单独的 popup 窗口：复用同一个 renderer / atlas，
//! 也省掉定位、失焦、多显示器翻转那一整套系统语义。

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, BoxShadow, Context, Div, MouseButton, MouseDownEvent,
    MouseMoveEvent, Pixels, Point, Stateful, Window, div, ease_out_quint, prelude::*, px,
};

use super::Spike;
use super::theme::{MENU_ITEM_HEIGHT, MENU_WIDTH, RADIUS_ITEM, RADIUS_PANEL, TITLEBAR_HEIGHT};
use super::widgets::mnemonic_label;
use crate::menu;
use crate::menu::{MenuItem, MenuSpec};

impl Spike {
    /// Alt+助记符：切换顶层菜单；未命中则收起已展开的菜单（同原生行为）
    pub(super) fn press_mnemonic(
        &mut self,
        key: char,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
            // 每次展开都换一个序号，让淡入动画重放
            self.menu_seq = self.menu_seq.wrapping_add(1);
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
        let palette = self.palette(window);
        let t = self.text();

        let window_width: f32 = window.viewport_size().width.into();
        let item_x = self
            .menu_rects
            .borrow()
            .get(index)
            .map(|rect| f32::from(rect.origin.x))
            .unwrap_or(8.0);
        // 贴右边缘时往左收，保证面板完整落在窗口里
        let panel_x = item_x.min((window_width - MENU_WIDTH - 8.0).max(8.0));

        let backdrop = self.menu_backdrop(cx);
        let panel = self
            .menu_panel(spec, panel_x, palette, t, cx)
            .with_animation(
                ("menu-panel", self.menu_seq),
                Animation::new(Duration::from_millis(120)).with_easing(ease_out_quint()),
                |panel, t| panel.opacity(t).mt(px((1.0 - t) * 4.0)),
            );

        Some(
            div()
                .absolute()
                .inset_0()
                .child(backdrop)
                .child(panel)
                .into_any_element(),
        )
    }

    /// 菜单展开时的全窗遮罩：负责关闭、顶层菜单切换与滚轮关闭
    fn menu_backdrop(&self, cx: &mut Context<Self>) -> Div {
        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
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
            }))
    }

    /// 下拉面板：容器样式 + 所有菜单条目
    fn menu_panel(
        &self,
        spec: &MenuSpec,
        panel_x: f32,
        palette: super::theme::Palette,
        t: crate::text::Text,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut panel = div()
            .absolute()
            .left(px(panel_x))
            .top(px(TITLEBAR_HEIGHT + 4.0))
            .w(px(MENU_WIDTH))
            .flex()
            .flex_col()
            .p(px(4.0))
            .rounded(px(RADIUS_PANEL))
            .bg(palette.menu_bg)
            .border_1()
            .border_color(palette.menu_border)
            .shadow(vec![
                BoxShadow::new(px(0.0), px(8.0), palette.shadow_strong)
                    .blur_radius(px(24.0))
                    .spread_radius(px(-6.0)),
                BoxShadow::new(px(0.0), px(1.0), palette.shadow_soft).blur_radius(px(3.0)),
            ])
            // 面板空白处也不要漏到遮罩上（否则点分隔线会关掉菜单）
            .occlude();

        for (index, item) in spec.items.iter().enumerate() {
            panel = match item {
                MenuItem::Separator => panel.child(
                    div()
                        .h(px(1.0))
                        .mx(px(8.0))
                        .my(px(4.0))
                        .bg(palette.separator),
                ),
                MenuItem::Command {
                    label,
                    shortcut,
                    mnemonic,
                    cmd,
                } => panel
                    .child(self.menu_row(index, *label, shortcut, *mnemonic, *cmd, palette, t, cx)),
            };
        }
        panel
    }

    /// 单个菜单条目；悬停同步键盘选中项，按回车执行的就是鼠标指向的命令
    fn menu_row(
        &self,
        index: usize,
        label: crate::text::TextKey,
        shortcut: &str,
        mnemonic: char,
        cmd: menu::Cmd,
        palette: super::theme::Palette,
        t: crate::text::Text,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let mut row = div()
            .id(("menu-row", index))
            .h(px(MENU_ITEM_HEIGHT))
            .px(px(8.0))
            .flex()
            .items_center()
            .rounded(px(RADIUS_ITEM))
            .text_size(px(12.0))
            .text_color(palette.text)
            .when(self.menu_selected == Some(index), |this| {
                this.bg(palette.active)
            })
            .hover(move |style| style.bg(palette.hover))
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                if this.menu_selected != Some(index) {
                    this.menu_selected = Some(index);
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.dispatch(cmd, window, cx);
            }))
            // 菜单展开时条目助记符始终可见（同系统菜单）
            .child(mnemonic_label(t.t(label), mnemonic, palette.text, true));

        if !shortcut.is_empty() {
            row = row.child(div().flex_1()).child(
                div()
                    .text_size(px(11.0))
                    .text_color(palette.text_muted)
                    .child(shortcut.to_string()),
            );
        }
        row
    }
}
