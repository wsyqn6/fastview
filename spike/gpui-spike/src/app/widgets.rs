//! 基础绘制件：窗口控制图标、窗口按钮、助记符文本
//!
//! gpui 的 `Path` 只有填充、没有描边，1px 线条得自己算顶点，因此图标全部几何自绘：
//! `Segoe Fluent Icons` 在 gpui 里匹配不到，字形会渲染成方框。

use gpui::{
    AnyElement, Div, HighlightStyle, Hsla, Path, Pixels, Point, StyledText, UnderlineStyle, Window,
    WindowControlArea, canvas, div, hsla, point, prelude::*, px, rgb,
};

use super::theme::{CLOSE_ACTIVE, CLOSE_HOVER, ICON_SIZE, Palette, WINDOW_BUTTON_WIDTH};
use crate::menu;

/// 窗口控制图标类型
#[derive(Clone, Copy)]
pub enum Icon {
    Minimize,
    Maximize,
    Restore,
    Close,
}

/// 把一条线段展开成有宽度的矩形，作为两个三角形推进 `Path`
fn push_stroke(path: &mut Path<Pixels>, from: Point<Pixels>, to: Point<Pixels>, width: f32) {
    let fx: f32 = from.x.into();
    let fy: f32 = from.y.into();
    let tx: f32 = to.x.into();
    let ty: f32 = to.y.into();
    let dx = tx - fx;
    let dy = ty - fy;
    let len = (dx * dx + dy * dy).sqrt();
    if len < f32::EPSILON {
        return;
    }

    let half = width / 2.0;
    let nx = -dy / len * half;
    let ny = dx / len * half;

    let a = point(px(fx + nx), px(fy + ny));
    let b = point(px(tx + nx), px(ty + ny));
    let c = point(px(tx - nx), px(ty - ny));
    let d = point(px(fx - nx), px(fy - ny));
    let st = (point(0.0f32, 0.0), point(1.0, 0.0), point(1.0, 1.0));
    path.push_triangle((a, b, c), st);
    path.push_triangle((a, c, d), st);
}

/// 描边矩形（四条边分别展开）
fn push_stroke_rect(path: &mut Path<Pixels>, left: f32, top: f32, right: f32, bottom: f32) {
    let width = 1.0;
    push_stroke(path, point(px(left), px(top)), point(px(right), px(top)), width);
    push_stroke(path, point(px(right), px(top)), point(px(right), px(bottom)), width);
    push_stroke(path, point(px(right), px(bottom)), point(px(left), px(bottom)), width);
    push_stroke(path, point(px(left), px(bottom)), point(px(left), px(top)), width);
}

/// 在给定中心绘制图标
///
/// 线条中心对到整数逻辑像素，1px 描边才不会发虚。
fn paint_icon(window: &mut Window, center: Point<Pixels>, icon: Icon, color: Hsla) {
    let cx: f32 = f32::from(center.x).round();
    let cy: f32 = f32::from(center.y).round();
    let mut path = Path::new(point(px(cx), px(cy)));

    match icon {
        Icon::Minimize => push_stroke(
            &mut path,
            point(px(cx - 5.0), px(cy)),
            point(px(cx + 5.0), px(cy)),
            1.0,
        ),
        // 描边中心在 4.5 时，外沿正好落在 ±5，整体 10×10
        Icon::Maximize => push_stroke_rect(&mut path, cx - 4.5, cy - 4.5, cx + 4.5, cy + 4.5),
        Icon::Restore => {
            // 前面那个窗口：左下方完整框
            push_stroke_rect(&mut path, cx - 4.5, cy - 2.0, cx + 2.5, cy + 5.0);
            // 后面那个窗口：只露上边与右边
            push_stroke(
                &mut path,
                point(px(cx - 2.0), px(cy - 4.5)),
                point(px(cx + 4.5), px(cy - 4.5)),
                1.0,
            );
            push_stroke(
                &mut path,
                point(px(cx + 4.5), px(cy - 4.5)),
                point(px(cx + 4.5), px(cy + 2.0)),
                1.0,
            );
        }
        Icon::Close => {
            push_stroke(
                &mut path,
                point(px(cx - 4.5), px(cy - 4.5)),
                point(px(cx + 4.5), px(cy + 4.5)),
                1.0,
            );
            push_stroke(
                &mut path,
                point(px(cx - 4.5), px(cy + 4.5)),
                point(px(cx + 4.5), px(cy - 4.5)),
                1.0,
            );
        }
    }

    window.paint_path(path, color);
}

/// 图标元素：统一尺寸与线宽，避免三个按钮看起来不是一套
fn icon_element(icon: Icon, color: Hsla) -> Div {
    div().w(px(ICON_SIZE)).h(px(ICON_SIZE)).child(
        canvas(
            |bounds, _, _| bounds.center(),
            move |_, center, window, _| paint_icon(window, center, icon, color),
        )
        .size_full(),
    )
}

/// Win11 风格的窗口控制按钮
///
/// 交给系统处理：`window_control_area` 会让 `WM_NCHITTEST` 返回 HTMINBUTTON /
/// HTMAXBUTTON / HTCLOSE，由 `DefWindowProc` 执行最小化、最大化（含 Snap Layouts）与关闭，
/// 因此这里不挂 on_click，避免和系统行为重复。
pub fn window_button(
    id: &'static str,
    icon: Icon,
    danger: bool,
    area: WindowControlArea,
    palette: Palette,
) -> AnyElement {
    let mut content = div()
        .relative()
        .w(px(ICON_SIZE))
        .h(px(ICON_SIZE))
        .child(icon_element(icon, palette.icon));

    if danger {
        // 悬停变红时图标转白：叠一层同形图标，靠 group_hover 淡入
        content = content.child(
            div()
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .opacity(0.0)
                .group_hover(id, |style| style.opacity(1.0))
                .child(icon_element(icon, hsla(0.0, 0.0, 1.0, 1.0))),
        );
    }

    div()
        .id(id)
        .group(id)
        .w(px(WINDOW_BUTTON_WIDTH))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .window_control_area(area)
        .hover(move |style| {
            if danger {
                style.bg(rgb(CLOSE_HOVER))
            } else {
                style.bg(palette.hover)
            }
        })
        .active(move |style| {
            if danger {
                style.bg(rgb(CLOSE_ACTIVE))
            } else {
                style.bg(palette.active)
            }
        })
        .child(content)
        .into_any_element()
}

/// 助记符文本：`show_underline` 时字母加下划线，其余样式继承父级
///
/// 下划线颜色必须显式给：`UnderlineStyle.color` 为 `None` 时回退到透明的默认色。
pub fn mnemonic_label(label: &str, mnemonic: char, color: Hsla, show_underline: bool) -> AnyElement {
    let (text, range) = menu::mnemonic_label(label, mnemonic);
    let styled = StyledText::new(text);
    if !show_underline {
        return styled.into_any_element();
    }

    let underline = HighlightStyle {
        underline: Some(UnderlineStyle {
            thickness: px(1.0),
            color: Some(color),
            wavy: false,
        }),
        ..Default::default()
    };

    styled
        .with_highlights([(range, underline)])
        .into_any_element()
}
