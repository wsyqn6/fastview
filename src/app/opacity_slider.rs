//! 不透明度滑杆：独立实体，拖动时只重绘自己，松手才通知父级应用

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    Bounds, Context, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, Render, WeakEntity,
    Window, div, prelude::*, px,
};

use super::Spike;
use super::theme::{Palette, RADIUS_ITEM};

/// 滑杆轨道宽度
const SLIDER_WIDTH: f32 = 180.0;
/// 滑杆圆点直径
const SLIDER_THUMB: f32 = 12.0;

/// 轨道局部 x → 百分比；越界钳到 0/100
fn value_at(x: f32, width: f32) -> Option<u8> {
    if width <= 0.0 {
        return None;
    }
    Some(((x / width).clamp(0.0, 1.0) * 100.0).round() as u8)
}

/// 高频交互只重绘这一小块，避免触发整窗重绘
pub(super) struct OpacitySlider {
    value: u8,
    palette: Palette,
    dragging: bool,
    bounds: Rc<RefCell<Option<Bounds<Pixels>>>>,
    parent: WeakEntity<Spike>,
}

impl OpacitySlider {
    pub(super) fn new(value: u8, palette: Palette, parent: WeakEntity<Spike>) -> Self {
        Self {
            value,
            palette,
            dragging: false,
            bounds: Rc::new(RefCell::new(None)),
            parent,
        }
    }

    pub(super) fn value(&self) -> u8 {
        self.value
    }

    pub(super) fn dragging(&self) -> bool {
        self.dragging
    }

    /// 父级每帧同步配色；拖动期间父级不重绘，配色也不会变
    pub(super) fn set_palette(&mut self, palette: Palette) {
        self.palette = palette;
    }

    /// 按指针位置更新本地值；只通知本实体
    pub(super) fn move_to(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(bounds) = *self.bounds.borrow() else {
            return;
        };
        let width: f32 = bounds.size.width.into();
        let x = f32::from(position.x - bounds.origin.x);
        let Some(value) = value_at(x, width) else {
            return;
        };
        if value == self.value {
            return;
        }
        self.value = value;
        cx.notify();
    }

    /// 结束拖动；返回最终值交给父级一次性应用
    pub(super) fn finish(&mut self) -> Option<u8> {
        std::mem::take(&mut self.dragging).then_some(self.value)
    }
}

impl Render for OpacitySlider {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let alpha = f32::from(self.value.min(100)) / 100.0;
        let bounds = self.bounds.clone();
        let parent = self.parent.clone();

        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(
                div()
                    .w(px(SLIDER_WIDTH))
                    .h(px(20.0))
                    .relative()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .on_children_prepainted(move |child_bounds, _, _| {
                        if let Some(first) = child_bounds.first() {
                            *bounds.borrow_mut() = Some(*first);
                        }
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            this.dragging = true;
                            this.move_to(event.position, cx);
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        if this.dragging {
                            this.move_to(event.position, cx);
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            // 先在本实体内收尾，避免父级再次读取正在借用的滑杆实体
                            let Some(value) = this.finish() else {
                                return;
                            };
                            if let Some(parent) = parent.upgrade() {
                                parent.update(cx, |spike, cx| {
                                    spike.apply_opacity_value(value, window, cx);
                                });
                            }
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
                            .bg(palette.accent)
                            .border_1()
                            .border_color(palette.card_border),
                    ),
            )
            .child(
                div()
                    .w(px(34.0))
                    .text_right()
                    .text_size(px(11.0))
                    .text_color(palette.text_muted)
                    .child(format!("{}%", self.value)),
            )
            .id("opacity-slider")
            .rounded(px(RADIUS_ITEM))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_at_maps_track_to_percent() {
        assert_eq!(value_at(0.0, 180.0), Some(0));
        assert_eq!(value_at(90.0, 180.0), Some(50));
        assert_eq!(value_at(180.0, 180.0), Some(100));
        assert_eq!(value_at(-10.0, 180.0), Some(0));
        assert_eq!(value_at(200.0, 180.0), Some(100));
        assert_eq!(value_at(0.0, 0.0), None);
    }
}
