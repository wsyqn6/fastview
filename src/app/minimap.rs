//! 大图导航缩略图（minimap）
//!
//! 只复用当前已解码纹理，不引入额外解码或缓存；几何全部由当前视口与查看状态推导。

use gpui::{
    AnyElement, Bounds, ObjectFit, Pixels, Point, Size, div, img, point, prelude::*, px, size,
};

use super::theme::{Palette, RADIUS_PANEL};
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
pub(super) fn should_show(image: &LoadedImage, view: Size<Pixels>, view_state: &ViewState, scale_factor: f32) -> bool {
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
fn highlight(base: Size<Pixels>, view: Size<Pixels>, view_state: &ViewState, layout: &MinimapLayout) -> Bounds<Pixels> {
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
    let inner_x: f32 = layout.inner_origin.x.into();
    let inner_y: f32 = layout.inner_origin.y.into();
    let inner_w: f32 = layout.inner.width.into();
    let inner_h: f32 = layout.inner.height.into();

    let width = (x1 - x0).max(MIN_HIGHLIGHT).min(inner_w);
    let height = (y1 - y0).max(MIN_HIGHLIGHT).min(inner_h);
    let center_x = (x0 + x1) / 2.0;
    let center_y = (y0 + y1) / 2.0;
    let x = (center_x - width / 2.0).clamp(inner_x, inner_x + inner_w - width);
    let y = (center_y - height / 2.0).clamp(inner_y, inner_y + inner_h - height);

    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

/// 渲染 minimap 卡片
pub(super) fn render(
    image: &LoadedImage,
    view: Size<Pixels>,
    view_state: &ViewState,
    scale_factor: f32,
    palette: &Palette,
) -> AnyElement {
    let base = image.logical_size(scale_factor);
    let map = layout(base, view);
    let mark = highlight(base, view, view_state, &map);

    div()
        .absolute()
        .left(map.origin.x)
        .top(map.origin.y)
        .w(map.card.width)
        .h(map.card.height)
        .rounded(px(RADIUS_PANEL))
        .bg(palette.card_bg)
        .border_1()
        .border_color(palette.card_border)
        .shadow_sm()
        .child(
            div()
                .absolute()
                .left(map.inner_origin.x)
                .top(map.inner_origin.y)
                .w(map.inner.width)
                .h(map.inner.height)
                .rounded(px(2.0))
                .overflow_hidden()
                .child(img(image.image.clone()).object_fit(ObjectFit::Fill).size_full())
                .child(
                    div()
                        .absolute()
                        .left(mark.origin.x)
                        .top(mark.origin.y)
                        .w(mark.size.width)
                        .h(mark.size.height)
                        .rounded(px(2.0))
                        .border(px(1.5))
                        .border_color(palette.accent)
                        .bg(palette.accent.opacity(0.10)),
                ),
        )
        .into_any_element()
}
