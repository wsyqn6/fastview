//! 设置浮层：主题 / 材质 / 透明度
//!
//! 与下拉菜单同一套做法：全窗遮罩负责关闭，面板自绘；改动即时生效并写盘。

use gpui::{
    AnyElement, BoxShadow, Context, Div, MouseButton, MouseDownEvent, Pixels, Point, Window,
    WindowBackgroundAppearance, div, hsla, prelude::*, px,
};

use super::Spike;
use super::settings::{Material, Settings, Theme};
use super::theme::{Palette, RADIUS_ITEM, RADIUS_PANEL};
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

        let backdrop = div()
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                    this.close_settings(window, cx);
                }),
            );

        // 主题分段
        let mut theme_seg = div().flex().gap(px(4.0));
        for (index, (label, value)) in [
            ("跟随系统", Theme::System),
            ("浅色", Theme::Light),
            ("深色", Theme::Dark),
        ]
        .into_iter()
        .enumerate()
        {
            let selected = self.settings.theme == value;
            theme_seg = theme_seg.child(
                div()
                    .id(("settings-theme", index))
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
                        next.theme = value;
                        this.update_settings(next, true, window, cx);
                    }))
                    .child(label),
            );
        }

        // 材质分段：Win10 没有系统 backdrop，云母选项无意义
        #[cfg(windows)]
        let mica_supported = self.hwnd != 0
            && win::supports_system_backdrop(self.hwnd as *mut core::ffi::c_void);
        #[cfg(not(windows))]
        let mica_supported = true;

        let mut material_options = vec![("亚克力", Material::Acrylic)];
        if mica_supported {
            material_options.push(("云母", Material::Mica));
        }
        material_options.push(("无模糊", Material::None));

        let mut material_seg = div().flex().gap(px(4.0));
        for (index, (label, value)) in material_options.into_iter().enumerate() {
            let selected = self.settings.material == value
                || (!mica_supported
                    && value == Material::Acrylic
                    && self.settings.material == Material::Mica);
            material_seg = material_seg.child(
                div()
                    .id(("settings-material", index))
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
                        next.material = value;
                        this.update_settings(next, true, window, cx);
                    }))
                    .child(label),
            );
        }

        // 透明度滑杆：与材质开关独立；100% 时材质只是不可见
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

        let opacity_control = div()
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
            );

        // 标题右侧显示当前组合，改完立刻能确认结果
        let theme_label = match self.settings.theme {
            Theme::System => "跟随系统",
            Theme::Light => "浅色",
            Theme::Dark => "深色",
        };
        let material_label = match self.settings.material {
            Material::Acrylic => "亚克力",
            Material::Mica if mica_supported => "云母",
            Material::Mica => "亚克力",
            Material::None => "无模糊",
        };

        let panel = div()
            .id("settings-panel")
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
                            .child("设置"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(palette.text_muted)
                            .child(format!(
                                "{theme_label} · {material_label} {}%",
                                self.settings.opacity
                            )),
                    ),
            )
            .child(settings_row(
                "主题",
                segmented(theme_seg, &palette),
                &palette,
            ))
            .child(settings_row(
                "窗口材质",
                segmented(material_seg, &palette),
                &palette,
            ))
            .child(settings_row(
                "不透明度",
                opacity_control.into_any_element(),
                &palette,
            ))
            .child(
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
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.close_settings(window, cx);
                        }))
                        .child("完成"),
                ),
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
fn settings_row(label: &'static str, control: AnyElement, palette: &Palette) -> Div {    div()
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
