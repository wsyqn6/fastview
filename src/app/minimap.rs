//! 大图导航缩略图（minimap）
//!
//! 只复用当前已解码纹理，不引入额外解码或缓存；几何全部由当前视口与查看状态推导。

use gpui::{
    AnyElement, Bounds, Context, MouseButton, MouseDownEvent, MouseMoveEvent, ObjectFit, Pixels,
    Point, Size, Window, div, img, point, prelude::*, px, size,
};

use super::Spike;
use super::theme::{Palette, RADIUS_PANEL, TITLEBAR_HEIGHT};
use crate::viewer::{LoadedImage, ViewState};

/// 卡片长边（逻辑像素）
const CARD_EDGE: f32 = 120.0;
/// 卡片内边距
const INNER_PADDING: f32 = 6.0;
/// 距内容区右下角
const MARGIN: f32 = 12.0;
/// 可视区高亮框最小边长
const MIN_HIGHLIGHT: f32 = 8.0;

/// minimap 几何：卡片位置、缩略图区域与图片->缩略图比例
pub(super) struct MinimapLayout {
    pub origin: Point<Pixels>,
    pub card: Size<Pixels>,
    pub inner_origin: Point<Pixels>,
    pub inner: Size<Pixels>,
    pub scale: f32,
}

/// 图片超出视口时才显示
pub(super) fn should_show(
    image: &LoadedImage,
    view: Size<Pixels>,
    view_state: &ViewState,
    scale_factor: f32,
) -> bool {
    let base = image.logical_size(scale_factor);
    let bounds = view_state.image_bounds(view, base);
    let image_w: f32 = bounds.size.width.into();
    let image_h: f32 = bounds.size.height.into();
    let view_w: f32 = view.width.into();
    let view_h: f32 = view.height.into();
    // 1px 容差，避免适应模式在浮点边界上闪烁
    image_w > view_w + 1.0 || image_h > view_h + 1.0
}

/// 从图片逻辑尺寸与视口尺寸推导布局
pub(super) fn layout(base: Size<Pixels>, view: Size<Pixels>) -> MinimapLayout {
    let image_w: f32 = base.width.into();
    let image_h: f32 = base.height.into();
    let thumb_edge = CARD_EDGE - 2.0 * INNER_PADDING;
    let (inner_w, inner_h) = if image_w >= image_h {
        (thumb_edge, thumb_edge * image_h / image_w.max(1.0))
    } else {
        (thumb_edge * image_w / image_h.max(1.0), thumb_edge)
    };
    let card_w = inner_w + 2.0 * INNER_PADDING;
    let card_h = inner_h + 2.0 * INNER_PADDING;
    let view_w: f32 = view.width.into();
    let view_h: f32 = view.height.into();

    let origin = point(
        px((view_w - MARGIN - card_w).max(0.0)),
        px((view_h - MARGIN - card_h).max(0.0)),
    );
    let origin_x: f32 = origin.x.into();
    let origin_y: f32 = origin.y.into();
    let inner_origin = point(px(origin_x + INNER_PADDING), px(origin_y + INNER_PADDING));

    MinimapLayout {
        origin,
        card: size(px(card_w), px(card_h)),
        inner_origin,
        inner: size(px(inner_w), px(inner_h)),
        scale: inner_w / image_w.max(1.0),
    }
}

/// 当前可视区在缩略图矩形中的位置
fn highlight(
    base: Size<Pixels>,
    view: Size<Pixels>,
    view_state: &ViewState,
    layout: &MinimapLayout,
) -> Bounds<Pixels> {
    let image_bounds = view_state.image_bounds(view, base);
    let zoom = view_state.scale(view, base);
    let view_w: f32 = view.width.into();
    let view_h: f32 = view.height.into();
    let image_x: f32 = image_bounds.origin.x.into();
    let image_y: f32 = image_bounds.origin.y.into();

    let left = image_bounds.origin.x.max(px(0.0));
    let top = image_bounds.origin.y.max(px(0.0));
    let right = (image_bounds.origin.x + image_bounds.size.width).min(px(view_w));
    let bottom = (image_bounds.origin.y + image_bounds.size.height).min(px(view_h));
    let left: f32 = left.into();
    let top: f32 = top.into();
    let right: f32 = right.into();
    let bottom: f32 = bottom.into();

    let x0 = (left - image_x) / zoom * layout.scale;
    let y0 = (top - image_y) / zoom * layout.scale;
    let x1 = (right - image_x) / zoom * layout.scale;
    let y1 = (bottom - image_y) / zoom * layout.scale;
    let inner_w: f32 = layout.inner.width.into();
    let inner_h: f32 = layout.inner.height.into();

    let width = (x1 - x0).max(MIN_HIGHLIGHT).min(inner_w);
    let height = (y1 - y0).max(MIN_HIGHLIGHT).min(inner_h);
    let center_x = (x0 + x1) / 2.0;
    let center_y = (y0 + y1) / 2.0;
    // x0/y0 已是缩略图局部坐标，clamp 范围也必须是局部的
    let x = (center_x - width / 2.0).clamp(0.0, inner_w - width);
    let y = (center_y - height / 2.0).clamp(0.0, inner_h - height);

    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

impl Spike {
    /// 把 minimap 上的窗口坐标换算成图片坐标，并移动视口
    pub(super) fn navigate_minimap(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(image) = self.image.as_ref() else {
            return;
        };
        let view = self.view_size(window);
        let base = image.logical_size(window.scale_factor());
        let map = layout(base, view);
        let chrome = if self.chrome_hidden(window) {
            0.0
        } else {
            TITLEBAR_HEIGHT
        };
        // 鼠标事件是窗口坐标，viewer 原点在标题栏下方
        let local = position - point(px(0.0), px(chrome));
        let x: f32 = local.x.into();
        let y: f32 = local.y.into();
        let inner_x: f32 = map.inner_origin.x.into();
        let inner_y: f32 = map.inner_origin.y.into();
        let base_w: f32 = base.width.into();
        let base_h: f32 = base.height.into();

        let image_x = ((x - inner_x) / map.scale).clamp(0.0, base_w);
        let image_y = ((y - inner_y) / map.scale).clamp(0.0, base_h);
        self.view
            .center_on(point(px(image_x), px(image_y)), view, base);
        cx.notify();
    }
}

/// 渲染 minimap 卡片
pub(super) fn render(
    image: &LoadedImage,
    view: Size<Pixels>,
    view_state: &ViewState,
    scale_factor: f32,
    palette: &Palette,
    cx: &mut Context<Spike>,
) -> AnyElement {
    let base = image.logical_size(scale_factor);
    let map = layout(base, view);
    let mark = highlight(base, view, view_state, &map);

    div()
        .id("minimap")
        .absolute()
        .left(map.origin.x)
        .top(map.origin.y)
        .w(map.card.width)
        .h(map.card.height)
        .rounded(px(RADIUS_PANEL))
        .shadow_md()
        .occlude()
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, event: &MouseDownEvent, window, cx| {
                this.minimap_drag = true;
                this.navigate_minimap(event.position, window, cx);
            }),
        )
        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
            if this.minimap_drag {
                this.navigate_minimap(event.position, window, cx);
            }
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                if std::mem::take(&mut this.minimap_drag) {
                    cx.notify();
                }
            }),
        )
        .child(
            div()
                .absolute()
                .left(px(INNER_PADDING))
                .top(px(INNER_PADDING))
                .w(map.inner.width)
                .h(map.inner.height)
                .rounded(px(2.0))
                .overflow_hidden()
                .child(
                    img(image.image.clone())
                        .object_fit(ObjectFit::Fill)
                        .size_full(),
                )
                .child(
                    div()
                        .absolute()
                        .left(mark.origin.x)
                        .top(mark.origin.y)
                        .w(mark.size.width)
                        .h(mark.size.height)
                        .rounded(px(2.0))
                        .border_1()
                        .border_color(palette.accent),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewer::ViewMode;

    #[test]
    fn highlight_uses_inner_local_coordinates() {
        let base = size(px(1000.0), px(1000.0));
        let view = size(px(400.0), px(400.0));
        let view_state = ViewState {
            mode: ViewMode::Manual,
            zoom: 2.0,
            ..ViewState::default()
        };
        let map = layout(base, view);
        let mark = highlight(base, view, &view_state, &map);

        let x: f32 = mark.origin.x.into();
        let y: f32 = mark.origin.y.into();
        let width: f32 = mark.size.width.into();
        let height: f32 = mark.size.height.into();
        let inner_w: f32 = map.inner.width.into();
        let inner_h: f32 = map.inner.height.into();

        assert!(
            x >= 0.0 && x + width <= inner_w + 0.01,
            "x={x}, inner_w={inner_w}"
        );
        assert!(
            y >= 0.0 && y + height <= inner_h + 0.01,
            "y={y}, inner_h={inner_h}"
        );
    }

    #[test]
    fn highlight_follows_view_offset() {
        let base = size(px(1000.0), px(1000.0));
        let view = size(px(400.0), px(400.0));
        let centered = ViewState {
            mode: ViewMode::Manual,
            zoom: 2.0,
            ..ViewState::default()
        };
        let mut shifted = ViewState {
            mode: ViewMode::Manual,
            zoom: 2.0,
            ..ViewState::default()
        };
        shifted.offset = point(px(-800.0), px(0.0));
        let map = layout(base, view);

        let a = highlight(base, view, &centered, &map);
        let b = highlight(base, view, &shifted, &map);
        let ax: f32 = a.origin.x.into();
        let bx: f32 = b.origin.x.into();

        assert!(ax > 0.0, "居中视口的高亮框不应贴在左边: {ax}");
        assert!((bx - ax).abs() > 1.0, "平移后高亮框应移动: {ax} -> {bx}");
    }
}
