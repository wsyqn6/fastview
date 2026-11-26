//! 键盘：快捷键与菜单命令落地

use gpui::{Context, KeyDownEvent, Window, point, px};

use super::Spike;
use super::info_panel::InfoKind;
use crate::menu;
use crate::viewer;

/// 只取单字符键，用来区分助记符与 `f11` / `space` 这类多字符键名
fn single_char(key: &str) -> Option<char> {
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}

impl Spike {
    /// 菜单展开时的键盘处理；返回 `true` 表示事件已被菜单消费
    fn handle_menu_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match key {
            "escape" => {
                self.close_menu();
                cx.notify();
                true
            }
            "down" => {
                self.move_menu_selection(true, cx);
                true
            }
            "up" => {
                self.move_menu_selection(false, cx);
                true
            }
            "right" => {
                self.switch_menu(true, cx);
                true
            }
            "left" => {
                self.switch_menu(false, cx);
                true
            }
            "enter" => {
                if let Some(cmd) = self.selected_cmd() {
                    self.dispatch(cmd, window, cx);
                }
                true
            }
            // 菜单展开时直接按助记符执行（原生行为），否则收起后按普通快捷键处理
            _ => {
                let cmd = single_char(key).and_then(|mnemonic| {
                    let index = self.open_menu?;
                    menu::cmd_by_mnemonic(index, mnemonic)
                });
                if let Some(cmd) = cmd {
                    self.dispatch(cmd, window, cx);
                } else {
                    self.close_menu();
                    cx.notify();
                }
                true
            }
        }
    }

    /// 键盘快捷键
    pub(super) fn handle_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        let key = keystroke.key.as_str();
        let modifiers = keystroke.modifiers;

        // 任何按键都算交互：状态栏重新出现并重置淡出计时
        self.touch_status(cx);

        if self.handle_overlay_key(key, modifiers, window, cx) {
            return;
        }

        // Alt+助记符直达顶层菜单（AltGr 是 Ctrl+Alt，必须排除）
        if modifiers.alt
            && !modifiers.control
            && !modifiers.platform
            && let Some(mnemonic) = single_char(key)
        {
            cx.stop_propagation();
            self.press_mnemonic(mnemonic, window, cx);
            return;
        }

        // 菜单打开时先由菜单消费键盘
        if self.open_menu.is_some() {
            self.handle_menu_key(key, window, cx);
            return;
        }

        // 单键快捷键只在无修饰键时生效，避免 Alt+数字 等被误当成缩放
        let plain = !modifiers.control && !modifiers.alt && !modifiers.platform;
        if self.handle_global_key(key, modifiers, plain, window, cx) {
            return;
        }
        self.handle_image_key(key, modifiers, plain, window, cx);
    }

    /// 浮层优先消费键盘；返回 true 表示本次按键已处理完毕
    fn handle_overlay_key(
        &mut self,
        key: &str,
        modifiers: gpui::Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        // 设置面板：Ctrl+, 开关，Esc 关闭，其余按键不落到查看器
        if modifiers.control && !modifiers.alt && (key == "," || key == "comma") {
            cx.stop_propagation();
            self.toggle_settings(window, cx);
            return true;
        }
        if self.settings_open {
            if key.eq_ignore_ascii_case("escape") {
                self.close_settings(window, cx);
            }
            return true;
        }
        // 快捷键说明：Esc / H / ? 关闭，其余按键不落到查看器
        if self.help_open {
            if key.eq_ignore_ascii_case("escape")
                || key.eq_ignore_ascii_case("h")
                || key == "?"
                || (modifiers.shift && key == "/")
            {
                self.close_help(cx);
            }
            return true;
        }
        // 关于 / 检查更新
        if self.info.is_some() {
            if key.eq_ignore_ascii_case("escape") || key.eq_ignore_ascii_case("enter") {
                self.close_info(cx);
            }
            return true;
        }
        // 照片信息：Esc 或 I 关闭（与打开时的 I 对称），其余按键不落到查看器
        if self.photo_info.is_some() {
            let plain = !modifiers.control && !modifiers.alt && !modifiers.platform;
            if key.eq_ignore_ascii_case("escape") || (plain && key.eq_ignore_ascii_case("i")) {
                self.close_photo_info(cx);
            }
            return true;
        }
        false
    }

    /// 不依赖当前图片的全局快捷键；返回 true 表示已处理
    fn handle_global_key(
        &mut self,
        key: &str,
        modifiers: gpui::Modifiers,
        plain: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if key.eq_ignore_ascii_case("escape") {
            if window.is_fullscreen() {
                self.toggle_fullscreen(window, cx);
            }
            return true;
        }
        if modifiers.control && !modifiers.alt && key.eq_ignore_ascii_case("o") {
            self.open_file(cx);
            return true;
        }
        if plain && (key.eq_ignore_ascii_case("f11") || key.eq_ignore_ascii_case("f")) {
            self.toggle_fullscreen(window, cx);
            return true;
        }
        if plain && key.eq_ignore_ascii_case("v") {
            self.toggle_chrome(cx);
            return true;
        }
        if plain && (key.eq_ignore_ascii_case("h") || key == "?" || (modifiers.shift && key == "/"))
        {
            self.toggle_help(cx);
            return true;
        }
        if plain && key.eq_ignore_ascii_case("i") {
            self.toggle_photo_info(window, cx);
            return true;
        }
        false
    }

    /// 当前图片相关的快捷键
    fn handle_image_key(
        &mut self,
        key: &str,
        modifiers: gpui::Modifiers,
        plain: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.image.is_none() || !plain {
            return;
        }
        match key {
            "0" => self.set_mode(viewer::ViewMode::Fit, window),
            "1" => self.set_mode(viewer::ViewMode::Original, window),
            "2" => self.set_mode(viewer::ViewMode::Fill, window),
            k if k.eq_ignore_ascii_case("r") => self.rotate(!modifiers.shift, window, cx),
            "+" | "=" => self.zoom_step(1.25, window),
            "-" | "_" => self.zoom_step(0.8, window),
            "t" => self.toggle_thumbs(cx),
            "s" => self.toggle_status_pin(cx),
            "left" => self.step_image(false, window, cx),
            "right" => self.step_image(true, window, cx),
            _ => return,
        }
        cx.notify();
    }

    /// 切换缩放方式
    pub(super) fn set_mode(&mut self, mode: viewer::ViewMode, window: &Window) {
        if self.image.is_none() {
            return;
        }
        let view = self.view_size(window);
        let base = self.base_size(window);
        self.view.set_mode(mode, view, base);
    }

    /// 以视口中心为锚点缩放
    pub(super) fn zoom_step(&mut self, factor: f32, window: &Window) {
        if self.image.is_none() {
            return;
        }
        let view = self.view_size(window);
        let base = self.base_size(window);
        let view_w: f32 = view.width.into();
        let view_h: f32 = view.height.into();
        let cursor = point(px(view_w / 2.0), px(view_h / 2.0));
        self.view.zoom_at(factor, cursor, view, base);
    }

    /// 菜单命令落地（后续这里会长成真正的 app action）
    pub(super) fn dispatch(&mut self, cmd: menu::Cmd, window: &mut Window, cx: &mut Context<Self>) {
        use menu::Cmd;

        // 任何命令都会收起下拉
        self.open_menu = None;
        self.menu_selected = None;

        match cmd {
            Cmd::Exit => {
                window.defer(cx, |window, _| window.remove_window());
                return;
            }
            Cmd::Open => {
                self.open_file(cx);
                cx.notify();
                return;
            }
            Cmd::Fullscreen => self.toggle_fullscreen(window, cx),
            Cmd::Fit => self.set_mode(viewer::ViewMode::Fit, window),
            Cmd::Original => self.set_mode(viewer::ViewMode::Original, window),
            Cmd::Fill => self.set_mode(viewer::ViewMode::Fill, window),
            Cmd::RotateCw => self.rotate(true, window, cx),
            Cmd::RotateCcw => self.rotate(false, window, cx),
            Cmd::Thumbnails => {
                self.toggle_thumbs(cx);
                return;
            }
            Cmd::Settings => {
                self.open_settings(cx);
                return;
            }
            Cmd::Shortcuts => {
                self.open_help(cx);
                return;
            }
            Cmd::PhotoInfo => {
                self.toggle_photo_info(window, cx);
                return;
            }
            Cmd::About => {
                self.open_info(InfoKind::About, cx);
                return;
            }
            Cmd::CheckUpdate => {
                #[cfg(windows)]
                crate::win::open_url(crate::win::RELEASES_URL);
                #[cfg(not(windows))]
                eprintln!("[SPIKE] releases: {}", crate::win::RELEASES_URL);
                return;
            }
        }

        eprintln!("[SPIKE] command {cmd:?}");
        cx.notify();
    }
}
