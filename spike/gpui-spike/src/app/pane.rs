//! 内容区：图片绘制与缩放 / 平移交互

use gpui::{
    AnyElement, Context, MouseButton, MouseDownEvent, MouseMoveEvent, ObjectFit, ScrollDelta,
    ScrollEvent, Size, Window, WindowControlArea, div, hsla, img, point, prelude::*, px,
};

use super::Spike;
use super::theme::{Palette, TITLEBAR_HEIGHT};
use crate::viewer;

impl Spike {
    pub(super) fn render_viewer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette(window);
        let view = self.view_size(window);
        let scale_factor = window.scale_factor();

        let mut area = div()
            .id("viewer")
            .relative()
            .flex_1()
            .overflow_hidden()
            .bg(palette.content_bg)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, _| {
                    this.drag_last = Some(event.position);
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                this.touch_status(cx);
                if !event.dragging() {
                    this.drag_last = None;
                    return;
                }
                let Some(last) = this.drag_last else {
                    this.drag_last = Some(event.position);
                    return;
                };
                let delta = event.position - last;
                this.drag_last = Some(event.position);

                if this.image.is_some() {
                    let view = this.view_size(window);
                    let base = this.base_size(window);
                    this.view.pan(delta, view, base);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag_last = None),
            )
            .on_scroll(cx.listener(|this, event: &ScrollEvent, window, cx| {
                if this.image.is_none() {
                    return;
                }
                this.touch_status(cx);
                let view = this.view_size(window);
                let base = this.base_size(window);
                let dy = match event.delta {
                    ScrollDelta::Pixels(delta) => delta.y.into(),
                    // 一行约 30 逻辑像素，一格滚轮（约 3 行）≈ 25%
                    ScrollDelta::Lines(delta) => delta.y * 30.0,
                };
                if dy == 0.0 {
                    return;
                }
                // 每 1 逻辑像素滚动放大约 0.25%
                let factor = (dy * 0.0025).exp();
                let chrome = if this.chrome_hidden(window) {
                    0.0
                } else {
                    TITLEBAR_HEIGHT
                };
                let cursor = event.position - point(px(0.0), px(chrome));
                this.view.zoom_at(factor, cursor, view, base);
                cx.notify();
            }));

        if let Some(image) = &self.image {
            area = area.child(self.render_image_layer(image, view, scale_factor));
            if !self.chrome_hidden(window) {
                area = area.child(self.render_status_bar(image, view, scale_factor, &palette));
            }
        } else {
            area = area.child(self.render_placeholder(&palette));
        }
        if self.hide_chrome {
            // 没有标题栏时保留顶部窄条拖动窗口，不占用图片平移区
            area = area.child(
                div()
                    .id("borderless-drag")
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .right(px(0.0))
                    .h(px(8.0))
                    .window_control_area(WindowControlArea::Drag),
            );
        }

        area.into_any_element()
    }

    /// 图片层：按当前缩放/平移放置
    fn render_image_layer(
        &self,
        image: &viewer::LoadedImage,
        view: Size<gpui::Pixels>,
        scale_factor: f32,
    ) -> AnyElement {
        let base = image.logical_size(scale_factor);
        let bounds = self.view.image_bounds(view, base);

        div()
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height)
            .child(img(image.image.clone()).object_fit(ObjectFit::Fill).size_full())
            .into_any_element()
    }

    /// 无图 / 加载中 / 出错时的占位
    fn render_placeholder(&self, palette: &Palette) -> AnyElement {
        let (title, hint) = if self.loading {
            ("加载中…".to_string(), String::new())
        } else if let Some(error) = &self.error {
            (error.clone(), String::new())
        } else {
            (
                "拖入图片，或按 Ctrl+O 打开".to_string(),
                "支持 JPEG / PNG / WebP / GIF / BMP / TIFF".to_string(),
            )
        };
        let title_color = if self.error.is_some() {
            hsla(0.0, 0.72, 0.60, 1.0)
        } else {
            palette.text
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(title_color)
                    .child(title),
            )
            .when(!hint.is_empty(), |this| {
                this.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(palette.text_muted)
                        .child(hint),
                )
            })
            .into_any_element()
    }
}
