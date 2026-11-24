//! 设置浮层：主题 / 材质 / 透明度
//!
//! 与下拉菜单同一套做法：全窗遮罩负责关闭，面板自绘；改动即时生效并写盘。

use gpui::{
    AnyElement, BoxShadow, Context, Div, MouseButton, MouseDownEvent, Pixels, Point, Window,
    WindowBackgroundAppearance, div, hsla, prelude::*, px,
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
/// 透明度滑杆轨道宽度
const SLIDER_WIDTH: f32 = 180.0;
/// 滑杆圆点直径
const SLIDER_THUMB: f32 = 12.0;

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
        self.settings_drag = false;
        self.settings.save();
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

    /// 拖动透明度滑杆：只改值，松手时才写盘
    pub(super) fn set_opacity_at(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bounds) = *self.slider_bounds.borrow() else {
            return;
        };
        let width = f32::from(bounds.size.width);
        if width <= 0.0 {
            return;
        }
        let x = f32::from(position.x - bounds.origin.x);
        let value = ((x / width).clamp(0.0, 1.0) * 100.0).round() as u8;
        if value == self.settings.opacity {
            return;
        }
        let was_translucent = super::translucent(&self.settings);
        self.settings.opacity = value;
        self.sync_window_background(was_translucent, window, cx);
        cx.notify();
    }

    /// 结束滑杆拖动并写盘（鼠标可能在滑杆外松开，由根元素接管）
    pub(super) fn finish_opacity_drag(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.settings_drag) {
            self.settings.save();
            cx.notify();
        }
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

        let panel = overlay_panel("settings-panel", PANEL_WIDTH, palette)
            .shadow(vec![
                BoxShadow::new(px(0.0), px(8.0), hsla(0.0, 0.0, 0.0, 0.24))
                    .blur_radius(px(24.0))
                    .spread_radius(px(-6.0)),
                BoxShadow::new(px(0.0), px(1.0), hsla(0.0, 0.0, 0.0, 0.14)).blur_radius(px(3.0)),
            ])
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
                self.opacity_control(palette, cx).into_any_element(),
                &palette,
            )
            .into_any_element(),
            self.toggle_section(t, palette, cx).into_any_element(),
        ]
    }

    /// 透明度滑杆：与材质开关独立；100% 时材质只是不可见
    fn opacity_control(&self, palette: Palette, cx: &mut Context<Self>) -> Div {
        let alpha = f32::from(self.settings.opacity.min(100)) / 100.0;
        let slider_bounds = self.slider_bounds.clone();
        let slider = div()
            .w(px(SLIDER_WIDTH))
            .h(px(20.0))
            .relative()
            .flex()
            .items_center()
            .cursor_pointer()
            .on_children_prepainted(move |bounds, _, _| {
                if let Some(bounds) = bounds.first() {
                    *slider_bounds.borrow_mut() = Some(*bounds);
                }
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.settings_drag = true;
                    this.set_opacity_at(event.position, window, cx);
                }),
            )
            .child(
                div()
                    .w_full()
                    .h(px(4.0))
                    .rounded_full()
                    .bg(palette.track)
                    .child(
                        div()
                            .h_full()
                            .w(px(SLIDER_WIDTH * alpha))
                            .rounded_full()
                            .bg(palette.accent),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .left(px((SLIDER_WIDTH - SLIDER_THUMB) * alpha))
                    .top(px((20.0 - SLIDER_THUMB) / 2.0))
                    .size(px(SLIDER_THUMB))
                    .rounded_full()
                    .bg(palette.accent),
            );

        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(slider)
            .child(
                div()
                    .w(px(34.0))
                    .text_right()
                    .text_size(px(11.0))
                    .text_color(palette.text_muted)
                    .child(format!("{}%", self.settings.opacity)),
            )
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
            .text_color(hsla(0.0, 0.0, 1.0, 1.0))
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
                        .bg(hsla(0.0, 0.0, 1.0, 1.0)),
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
