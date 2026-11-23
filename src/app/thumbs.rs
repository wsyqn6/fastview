//! 缩略图条：加载系统缩略图、横向排布、点击切换

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, AnyElement, Context, ObjectFit, RenderImage, ScrollEvent, Window, div,
    ease_out_quint, img, point, prelude::*, px,
};

use super::Spike;
use super::theme::{THUMB_BAR_HEIGHT, THUMB_SIZE};
use crate::{folder, shell};

/// 向系统要的缩略图长边（留 HiDPI 余量，实际尺寸由 shell 决定）
const THUMB_EDGE: u32 = 160;
/// 单个目录最多加载多少张缩略图
const MAX_THUMBS: usize = 512;
/// 统一高度（逻辑像素），宽度按原图比例
const THUMB_HEIGHT: f32 = 48.0;
/// 相框最大内容宽度，超出则按宽度反算高度
const THUMB_MAX_WIDTH: f32 = THUMB_SIZE - 8.0;
/// 相框边框宽度（两侧）
const FRAME_BORDER: f32 = 1.0;
/// 每项布局宽度（格子 + gap），用于虚拟化时计算滚动偏移
const ITEM_STRIDE: f32 = THUMB_SIZE + 8.0;
/// 可视区前后多渲染几项，避免滚动时出现空白
const OVERSCAN: usize = 3;

/// 缩略图显示尺寸：高度统一、宽度随原图比例，超宽时收窄
fn display_size(image: &RenderImage) -> (f32, f32) {
    let size = image.size(0);
    let w = size.width.0.max(1) as f32;
    let h = size.height.0.max(1) as f32;
    let mut width = THUMB_HEIGHT * w / h;
    let mut height = THUMB_HEIGHT;
    if width > THUMB_MAX_WIDTH {
        width = THUMB_MAX_WIDTH;
        height = width * h / w;
    }
    (width, height)
}

impl Spike {
    /// 让目录列表跟上当前文件：同目录只移动游标，换目录则重扫并重载缩略图
    pub(super) fn sync_folder(
        &mut self,
        path: &std::path::Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let parent = path.parent();
        if let Some(folder) = self.folder.as_mut()
            && Some(folder.dir.as_path()) == parent
        {
            folder.select(path);
            return;
        }

        // 换目录：旧缩略图纹理必须显式回收
        for (_, image) in self.thumbs.drain() {
            let _ = window.drop_image(image);
        }
        self.thumb_gen = self.thumb_gen.wrapping_add(1);
        self.folder = folder::Folder::scan(path);

        if self.show_thumbs {
            let generation = self.thumb_gen;
            self.load_thumbnails(generation, cx);
        }
    }

    /// 后台串行加载缩略图：shell 首次生成慢，命中缓存只要几毫秒
    fn load_thumbnails(&mut self, generation: u64, cx: &mut Context<Self>) {
        let Some(folder) = self.folder.as_ref() else {
            return;
        };
        // 目录过大时只做前一段，避免长时间占用后台线程
        let files: Vec<PathBuf> = folder.files.iter().take(MAX_THUMBS).cloned().collect();
        let total = files.len();

        cx.spawn(async move |this, cx| {
            let started = Instant::now();
            let mut loaded = 0usize;

            for path in files {
                let task = path.clone();
                let thumb = cx
                    .background_executor()
                    .spawn(async move { shell::thumbnail(&task, THUMB_EDGE) })
                    .await;
                let Ok(thumb) = thumb else {
                    continue;
                };
                let Some(buffer) = image::RgbaImage::from_raw(thumb.width, thumb.height, thumb.bgra)
                else {
                    continue;
                };

                loaded += 1;
                let keep_going = this
                    .update(cx, |this, cx| {
                        // 已换目录，丢弃结果
                        if this.thumb_gen != generation {
                            return false;
                        }
                        let texture = Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]));
                        this.thumbs.insert(path, texture);
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !keep_going {
                    return;
                }
            }

            eprintln!(
                "[SPIKE] thumbnails {loaded}/{total} in {} ms",
                started.elapsed().as_millis()
            );
        })
        .detach();
    }

    /// 显示 / 隐藏缩略图条；首次打开时才去碰系统缓存
    pub(super) fn toggle_thumbs(&mut self, cx: &mut Context<Self>) {
        self.show_thumbs = !self.show_thumbs;
        if self.show_thumbs && self.thumbs.is_empty() {
            let generation = self.thumb_gen;
            self.load_thumbnails(generation, cx);
        }
        self.settings.show_thumbs = self.show_thumbs;
        self.settings.save();
        cx.notify();
    }

    /// 缩略图条：底部横向排布，点击切换
    pub(super) fn render_thumbnails(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(folder) = self.folder.as_ref() else {
            return div().into_any_element();
        };
        let palette = self.palette(window);
        let current = folder.index;
        let total = folder.files.len().min(MAX_THUMBS);

        let view_width: f32 = self.thumb_scroll.bounds().size.width.into();
        // gpui 的 scroll offset 为负：向左滚得越远越负，取反得到正向滚动距离
        let offset = self.thumb_scroll.offset();
        let scrolled: f32 = -f32::from(offset.x);

        // 换图后把当前项滚进可视区；首帧视口宽为 0 时等下一帧再定位
        if self.thumb_scrolled != Some(current) && view_width > 0.0 {
            let item_left = current as f32 * ITEM_STRIDE;
            let item_right = item_left + THUMB_SIZE;
            let next = if item_left < scrolled {
                item_left
            } else if item_right > scrolled + view_width {
                item_right - view_width
            } else {
                scrolled
            };
            self.thumb_scroll.set_offset(point(px(-next), offset.y));
            self.thumb_scrolled = Some(current);
        }

        // 只构造可视区 ± OVERSCAN 的 item；前后 spacer 撑出完整滚动宽度
        let scrolled: f32 = -f32::from(self.thumb_scroll.offset().x);
        let first = (scrolled / ITEM_STRIDE) as usize;
        let visible = ((view_width / ITEM_STRIDE).ceil() as usize + 1).max(8);
        let start = first.saturating_sub(OVERSCAN);
        let end = (first + visible + OVERSCAN).min(total);

        let items = folder
            .files
            .iter()
            .skip(start)
            .take(end.saturating_sub(start))
            .enumerate()
            .map(|(offset_index, path)| {
                let index = start + offset_index;
                let selected = index == current;
                let target = path.clone();
                let image = self.thumbs.get(path).cloned();
                let loaded = image.is_some();
                div()
                    .id(("thumb", index))
                    .flex_none()
                    .size(px(THUMB_SIZE))
                    .mr(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .relative()
                    .rounded(px(6.0))
                    .when(selected, |this| this.bg(palette.active))
                    .when(!selected, |this| {
                        this.hover(move |style| style.bg(palette.hover))
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_path(target.clone(), window, cx);
                    }))
                    // 加载前只留中性占位，加载后才出现按比例相框，避免尺寸跳变
                    .when_some(image, |this, image| {
                        let (w, h) = display_size(&image);
                        this.child(
                            div()
                                .w(px(w + 2.0 * FRAME_BORDER))
                                .h(px(h + 2.0 * FRAME_BORDER))
                                .rounded(px(4.0))
                                .overflow_hidden()
                                .bg(palette.hover)
                                .border_1()
                                .border_color(palette.separator)
                                .child(img(image).object_fit(ObjectFit::Fill).size_full()),
                        )
                    })
                    .when(!loaded, |this| {
                        this.child(
                            div()
                                .size(px(16.0))
                                .rounded(px(3.0))
                                .bg(palette.separator.opacity(0.6)),
                        )
                    })
                    .when(selected, |this| {
                        this.child(
                            div()
                                .absolute()
                                .bottom(px(2.0))
                                .left(px(16.0))
                                .right(px(16.0))
                                .h(px(2.0))
                                .rounded_full()
                                .bg(palette.accent),
                        )
                    })
                    .into_any_element()
            });

        div()
            .id("thumbbar")
            .flex_none()
            .h(px(THUMB_BAR_HEIGHT))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .overflow_hidden()
            .bg(palette.surface_bg)
            .child(
                div()
                    .id("thumbbar-scroll")
                    .size_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .px_2()
                    .overflow_hidden()
                    .overflow_x_scroll()
                    .track_scroll(&self.thumb_scroll)
                    // 滚动会改变可见范围，必须触发 render 补上新进入的 item
                    .on_scroll(cx.listener(|_, _: &ScrollEvent, _, cx| cx.notify()))
                    .child(div().flex_none().w(px(start as f32 * ITEM_STRIDE)))
                    .children(items)
                    .child(div().flex_none().w(px((total - end) as f32 * ITEM_STRIDE)))
                    // 内容淡入 + 轻微上移；外层高度不变，查看器不会跟着抖动
                    .with_animation(
                        "thumbbar-open",
                        Animation::new(Duration::from_millis(160)).with_easing(ease_out_quint()),
                        |row, t| row.opacity(t).mt(px((1.0 - t) * 6.0)),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texture(width: u32, height: u32) -> RenderImage {
        let buffer =
            image::RgbaImage::from_raw(width, height, vec![0; (width * height * 4) as usize])
                .expect("尺寸与缓冲一致");
        RenderImage::new(vec![image::Frame::new(buffer)])
    }

    #[test]
    fn display_size_square() {
        let (w, h) = display_size(&texture(100, 100));
        assert!((w - THUMB_HEIGHT).abs() < 0.01);
        assert!((h - THUMB_HEIGHT).abs() < 0.01);
    }

    #[test]
    fn display_size_wide_clamped_by_max_width() {
        let (w, h) = display_size(&texture(192, 108));
        assert!((w - THUMB_MAX_WIDTH).abs() < 0.01);
        assert!((h - 31.5).abs() < 0.01);
    }

    #[test]
    fn display_size_portrait_keeps_height() {
        let (w, h) = display_size(&texture(108, 192));
        assert!((w - 27.0).abs() < 0.01);
        assert!((h - THUMB_HEIGHT).abs() < 0.01);
    }
}
