//! 缩略图条：加载系统缩略图、横向排布、点击切换

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, AnyElement, Context, ObjectFit, RenderImage, Window, div,
    ease_out_quint, hsla, img, prelude::*, px,
};

use super::Spike;
use super::theme::{THUMB_BAR_HEIGHT, THUMB_SIZE};
use crate::{folder, shell};

/// 向系统要的缩略图长边（留 HiDPI 余量，实际尺寸由 shell 决定）
const THUMB_EDGE: u32 = 160;
/// 单个目录最多加载多少张缩略图
const MAX_THUMBS: usize = 512;

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

        // 换图后把当前项滚进可视区（目录几十张时看不到高亮就没意义了）
        if self.thumb_scrolled != Some(current) {
            self.thumb_scroll.scroll_to_item(current);
            self.thumb_scrolled = Some(current);
        }

        let items = folder
            .files
            .iter()
            .take(MAX_THUMBS)
            .enumerate()
            .map(|(index, path)| {
                let selected = index == current;
                let target = path.clone();
                div()
                    .id(("thumb", index))
                    .flex_none()
                    .size(px(THUMB_SIZE))
                    // 选中环 2px + 2px 间隙，未选中也保留同宽透明边框，避免跳动
                    .border_2()
                    .border_color(if selected {
                        palette.accent
                    } else {
                        hsla(0.0, 0.0, 0.0, 0.0)
                    })
                    .p(px(2.0))
                    .rounded(px(6.0))
                    .when(!selected, |this| {
                        this.hover(move |style| style.bg(palette.hover))
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_path(target.clone(), window, cx);
                    }))
                    .when_some(self.thumbs.get(path).cloned(), |this, image| {
                        this.child(
                            img(image)
                                .object_fit(ObjectFit::Contain)
                                .size_full()
                                .rounded(px(4.0)),
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
                    .gap_2()
                    .px_2()
                    .overflow_hidden()
                    .overflow_x_scroll()
                    .track_scroll(&self.thumb_scroll)
                    .children(items)
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
