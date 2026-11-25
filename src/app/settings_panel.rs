//! 设置浮层：主题 / 材质 / 透明度
//!
//! 与下拉菜单同一套做法：全窗遮罩负责关闭，面板自绘；改动即时生效并写盘。

use gpui::{
    AnyElement, BoxShadow, Context, Div, MouseButton, MouseMoveEvent, Window,
    WindowBackgroundAppearance, div, prelude::*, px,
};

use super::Spike;
use super::settings::{Language, Material, Settings, Theme};
use super::theme::{Palette, RADIUS_ITEM};
use super::widgets::{overlay, overlay_panel, panel_header};
use crate::text::{Text, TextKey};
#[cfg(windows)]
use crate::win;

/// 设置面板宽度
const PANEL_WIDTH: f32 = 420.0;

impl Spike {
    pub(super) fn open_settings(&mut self, cx: &mut Context<Self>) {
        if self.settings_open {
            return;
        }
        self.open_menu = None;
        self.menu_selected = None;
        self.help_open = false;
        self.info = None;
        self.photo_info = None;
        self.settings_open = true;
        cx.notify();
    }

    pub(super) fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.settings_open {
            return;
        }
        self.settings_open = false;
        self.commit_opacity(window, cx);
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    pub(super) fn toggle_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_open {
            self.close_settings(window, cx);
        } else {
            self.open_settings(cx);
        }
    }

    /// 100% 与其他档位之间切换时，窗口清屏方式要跟着变
    fn sync_window_background(&mut self, was_translucent: bool, window: &mut Window, cx: &mut Context<Self>) {
        let now_translucent = super::translucent(&self.settings);
        if was_translucent == now_translucent {
            return;
        }
        let background = if now_translucent {
            WindowBackgroundAppearance::Transparent
        } else {
            WindowBackgroundAppearance::Opaque
        };
        window.set_background_appearance(background);
        self.apply_window_style(window, cx);
    }

    /// 应用新配置：材质与透明度是两个独立维度
    fn update_settings(
        &mut self,
        next: Settings,
        persist: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous = self.settings.clone();
        let was_translucent = super::translucent(&previous);
        self.settings = next;

        let style_changed = previous.theme != self.settings.theme
            || previous.material != self.settings.material;
        self.sync_window_background(was_translucent, window, cx);
        if style_changed && was_translucent == super::translucent(&self.settings) {
            self.apply_window_style(window, cx);
        }
        if persist {
            self.settings.save();
        }
        cx.notify();
    }

    /// 取出滑杆最终值；只在结束时调用
    fn take_opacity_value(&mut self, cx: &mut Context<Self>) -> (Option<u8>, bool) {
        let dragging = self.opacity_slider.read(cx).dragging();
        let value = dragging
            .then(|| self.opacity_slider.update(cx, |slider, _| slider.finish()))
            .flatten();
        (value, dragging)
    }

    /// 应用已经取出的透明度值；不读取滑杆实体，避免子实体回调中重入
    pub(super) fn apply_opacity_value(
        &mut self,
        value: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if value != self.settings.opacity {
            let was_translucent = super::translucent(&self.settings);
            self.settings.opacity = value;
            self.sync_window_background(was_translucent, window, cx);
            cx.notify();
        }
        self.settings.save();
    }

    /// 从滑杆取值并提交；仅用于父级主动收尾路径
    pub(super) fn commit_opacity(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let (value, had_drag) = self.take_opacity_value(cx);
        if let Some(value) = value {
            self.apply_opacity_value(value, window, cx);
        } else if had_drag {
            self.settings.save();
        }
        had_drag
    }

    /// 无窗口上下文时（如切到帮助页）仅提交值与配置
    pub(super) fn commit_opacity_quiet(&mut self, cx: &mut Context<Self>) -> bool {
        let (value, had_drag) = self.take_opacity_value(cx);
        if let Some(value) = value {
            self.settings.opacity = value;
        }
        if had_drag {
            self.settings.save();
            cx.notify();
        }
        had_drag
    }

    pub(super) fn render_settings_panel(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.settings_open {
            return None;
        }
        let palette = self.palette(window);
        let t = self.text();
        let mica_supported = self.mica_supported();
        let summary = format!(
            "{} · {} {}%",
            theme_label(self.settings.theme, t),
            material_label(self.settings.material, t, mica_supported),
            self.settings.opacity
        );

        let panel = overlay_panel("settings-panel", PANEL_WIDTH, palette, window.viewport_size())
            .shadow(vec![
                BoxShadow::new(px(0.0), px(8.0), palette.shadow_strong)
                    .blur_radius(px(24.0))
                    .spread_radius(px(-6.0)),
                BoxShadow::new(px(0.0), px(1.0), palette.shadow_soft).blur_radius(px(3.0)),
            ])
            // 滑杆内部已接管拖动；这里兜底处理指针滑出滑杆但仍在本面板内的情况
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.opacity_slider.read(cx).dragging() {
                    let position = event.position;
                    this.opacity_slider
                        .update(cx, |slider, cx| slider.move_to(position, cx));
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.commit_opacity(window, cx);
                }),
            )
            .child(panel_header(t.t(TextKey::Settings), summary, palette))
            .children(self.settings_controls(t, palette, cx, mica_supported))
            .child(done_button(t, palette, cx));

        Some(overlay(panel.into_any_element(), cx, |this, window, cx| {
            this.close_settings(window, cx);
        }))
    }

    /// Win10 没有系统 backdrop，云母选项无意义
    fn mica_supported(&self) -> bool {
        #[cfg(windows)]
        {
            self.hwnd != 0 && win::supports_system_backdrop(self.hwnd as *mut core::ffi::c_void)
        }
        #[cfg(not(windows))]
        {
            true
        }
    }

    /// 设置面板的控件区：三个分段选择、透明度、开关区
    fn settings_controls(
        &mut self,
        t: Text,
        palette: Palette,
        cx: &mut Context<Self>,
        mica_supported: bool,
    ) -> Vec<AnyElement> {
        let language = segmented_control(
            "settings-language",
            vec![
                (t.t(TextKey::LanguageSystem), Language::System),
                (t.t(TextKey::LanguageChinese), Language::Chinese),
                (t.t(TextKey::LanguageEnglish), Language::English),
            ],
            |settings, value| settings.language = value,
            self.settings.language,
            palette,
            cx,
        );
        let theme = segmented_control(
            "settings-theme",
            vec![
                (t.t(TextKey::ThemeSystem), Theme::System),
                (t.t(TextKey::ThemeLight), Theme::Light),
                (t.t(TextKey::ThemeDark), Theme::Dark),
            ],
            |settings, value| settings.theme = value,
            self.settings.theme,
            palette,
            cx,
        );

        let mut materials = vec![(t.t(TextKey::MaterialAcrylic), Material::Acrylic)];
        if mica_supported {
            materials.push((t.t(TextKey::MaterialMica), Material::Mica));
        }
        materials.push((t.t(TextKey::MaterialNone), Material::None));
        let current_material = if !mica_supported && self.settings.material == Material::Mica {
            Material::Acrylic
        } else {
            self.settings.material
        };
        let material = segmented_control(
            "settings-material",
            materials,
            |settings, value| settings.material = value,
            current_material,
            palette,
            cx,
        );

        vec![
            settings_row(t.t(TextKey::Language), language, &palette).into_any_element(),
            settings_row(t.t(TextKey::Theme), theme, &palette).into_any_element(),
            settings_row(t.t(TextKey::Material), material, &palette).into_any_element(),
            settings_row(
                t.t(TextKey::Opacity),
                self.opacity_control(palette, cx),
                &palette,
            )
            .into_any_element(),
            self.toggle_section(t, palette, cx).into_any_element(),
        ]
    }

    /// 透明度滑杆：独立实体，拖动只重绘自身
    fn opacity_control(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        self.opacity_slider
            .update(cx, |slider, _| slider.set_palette(palette));
        self.opacity_slider.clone().into_any_element()
    }

    /// 开关区：点击复用快捷键的 toggle，状态与写盘逻辑只有一份
    fn toggle_section(&self, t: Text, palette: Palette, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(div().h(px(1.0)).bg(palette.separator))
            .child(toggle_row(
                t.t(TextKey::ThumbnailBar),
                self.settings.show_thumbs,
                "settings-thumbs",
                &palette,
                |this, cx| this.toggle_thumbs(cx),
                cx,
            ))
            .child(toggle_row(
                t.t(TextKey::StatusPinned),
                self.settings.status_pinned,
                "settings-status",
                &palette,
                |this, cx| this.toggle_status_pin(cx),
                cx,
            ))
            .child(toggle_row(
                t.t(TextKey::Borderless),
                self.settings.hide_chrome,
                "settings-chrome",
                &palette,
                |this, cx| this.toggle_chrome(cx),
                cx,
            ))
            .child(toggle_row(
                t.t(TextKey::AutoWindowSize),
                self.settings.auto_window_size,
                "settings-window-fit",
                &palette,
                |this, cx| this.toggle_auto_window_size(cx),
                cx,
            ))
    }
}

/// 通用分段控件：选中态由当前值判断，点击后只改配置的一个字段
fn segmented_control<T: Copy + PartialEq + 'static>(
    id: &'static str,
    options: Vec<(&'static str, T)>,
    write: fn(&mut Settings, T),
    current: T,
    palette: Palette,
    cx: &mut Context<Spike>,
) -> AnyElement {
    let mut segments = div().flex().gap(px(4.0));
    for (index, (label, value)) in options.into_iter().enumerate() {
        let selected = current == value;
        segments = segments.child(
            div()
                .id((id, index))
                .h(px(28.0))
                .px(px(10.0))
                .flex()
                .items_center()
                .rounded(px(RADIUS_ITEM + 2.0))
                .text_size(px(12.0))
                .text_color(if selected {
                    palette.text
                } else {
                    palette.text_muted
                })
                .when(selected, |this| this.bg(palette.active))
                .when(!selected, |this| {
                    this.hover(move |style| style.bg(palette.hover))
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    let mut next = this.settings.clone();
                    write(&mut next, value);
                    this.update_settings(next, true, window, cx);
                }))
                .child(label),
        );
    }
    segmented(segments, &palette)
}

/// 标题右侧的主题 / 材质摘要
fn theme_label(theme: Theme, t: Text) -> &'static str {
    match theme {
        Theme::System => t.t(TextKey::ThemeSystem),
        Theme::Light => t.t(TextKey::ThemeLight),
        Theme::Dark => t.t(TextKey::ThemeDark),
    }
}

fn material_label(material: Material, t: Text, mica_supported: bool) -> &'static str {
    match material {
        Material::Acrylic => t.t(TextKey::MaterialAcrylic),
        Material::Mica if mica_supported => t.t(TextKey::MaterialMica),
        Material::Mica => t.t(TextKey::MaterialAcrylic),
        Material::None => t.t(TextKey::MaterialNone),
    }
}

fn done_button(t: Text, palette: Palette, cx: &mut Context<Spike>) -> Div {
    div().flex().justify_end().child(
        div()
            .id("settings-done")
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
            .on_click(cx.listener(|this, _, window, cx| this.close_settings(window, cx)))
            .child(t.t(TextKey::Done)),
    )
}

/// 一行开关：点击复用快捷键的 toggle，状态与写盘逻辑只有一份
fn toggle_row(
    label: &'static str,
    on: bool,
    id: &'static str,
    palette: &Palette,
    on_click: impl Fn(&mut Spike, &mut Context<Spike>) + 'static,
    cx: &mut Context<Spike>,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .h(px(32.0))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(palette.text_muted)
                .child(label),
        )
        .child(
            div()
                .id(id)
                .w(px(36.0))
                .h(px(20.0))
                .rounded_full()
                .p(px(3.0))
                .bg(if on { palette.accent } else { palette.track })
                .flex()
                .when(on, |switch| switch.justify_end())
                .cursor_pointer()
                .hover(|style| style.opacity(0.9))
                .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
                .child(
                    div()
                        .w(px(14.0))
                        .h(px(14.0))
                        .rounded_full()
                        .bg(palette.on_accent),
                ),
        )
}

/// 分段控件外框：细底槽让「选中块」看起来是浮在上面的
fn segmented(segments: Div, palette: &Palette) -> AnyElement {
    div()
        .flex()
        .gap(px(2.0))
        .p(px(2.0))
        .rounded(px(RADIUS_ITEM + 3.0))
        .bg(palette.hover)
        .child(segments)
        .into_any_element()
}

/// 一行「标签 + 控件」
fn settings_row(label: &'static str, control: AnyElement, palette: &Palette) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .h(px(32.0))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(palette.text_muted)
                .child(label),
        )
        .child(control)
}
