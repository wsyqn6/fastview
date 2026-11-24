//! 图片加载：打开、分级解码、上屏与缓存、预取、补档
//!
//! 所有解码都在后台执行，主线程只负责换纹理与回收过期纹理。

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{Context, PathPromptOptions, RenderImage, Window};

use super::Spike;
use crate::archive::{self, ExtractError};
use crate::decode::{self, ImageMeta, Preview, Rotation};
use crate::text::TextKey;
use crate::{folder, viewer};

/// 后台 probe + 解码的结果
struct Decoded {
    meta: ImageMeta,
    preview: Preview,
    elapsed: Duration,
}

/// 解码排队期间generation已变则跳过，省掉无用的后台占用
fn stale<C: gpui::AppContext>(this: &gpui::WeakEntity<Spike>, cx: &C, load_id: u64) -> bool {
    this.read_with(cx, |this, _| this.load_id != load_id)
        .unwrap_or(true)
}

/// 统一的后台解码入口：已有 meta 时跳过 probe，预览按需选档
async fn decode_task(
    path: PathBuf,
    need_px: u32,
    rotation: Rotation,
    known_meta: Option<ImageMeta>,
) -> Result<Decoded, String> {
    let started = Instant::now();
    let meta = match known_meta {
        Some(meta) => meta,
        None => decode::probe(&path)?,
    };
    let preview = decode::load_preview(&path, &meta, need_px, rotation)?;
    Ok(Decoded {
        meta,
        preview,
        elapsed: started.elapsed(),
    })
}

impl Spike {
    /// 打开文件对话框（系统原生），选中后在后台解码
    pub(super) fn open_file(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(self.text().t(TextKey::OpenPrompt).into()),
        });

        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update_in(cx, |this, window, cx| this.open_source(path, window, cx))
                .ok();
        })
        .detach();
    }

    /// 外部路径统一入口：图片、目录、zip 分流
    pub(super) fn open_source(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match folder::input_kind(&path) {
            folder::InputKind::Image => {
                // 包内翻页仍属于当前 zip，不能提前删掉临时目录
                if !self
                    .archive
                    .as_ref()
                    .is_some_and(|archive| archive.contains(&path))
                {
                    self.archive = None;
                }
                self.open_path(path, window, cx);
            }
            folder::InputKind::Directory => {
                self.archive = None;
                let Some(folder) = folder::Folder::scan_dir(&path) else {
                    let message = self.text().t(TextKey::NoImages);
                    self.show_error(message, cx);
                    return;
                };
                let first = folder
                    .current()
                    .expect("scanned folder has images")
                    .to_path_buf();
                self.open_path(first, window, cx);
            }
            folder::InputKind::Archive => self.open_archive(path, window, cx),
        }
    }

    /// zip 解压放到后台，完成后复用普通图片的打开流程
    fn open_archive(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(first) = self
            .archive
            .as_ref()
            .filter(|archive| archive.source == path)
            .map(|archive| archive.first.clone())
        {
            self.open_path(first, window, cx);
            return;
        }

        self.load_id = self.load_id.wrapping_add(1);
        self.loading = true;
        self.error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { archive::extract_images(&path) })
                .await;
            this.update_in(cx, |this, window, cx| match result {
                Ok(archive) => {
                    let first = archive.first.clone();
                    this.archive = Some(archive);
                    this.open_path(first, window, cx);
                }
                Err(ExtractError::NoImages) => {
                    let message = this.text().t(TextKey::NoImages);
                    this.show_error(message, cx);
                }
                Err(ExtractError::Failed(message)) => {
                    eprintln!("[SPIKE] open archive failed: {message}");
                    let message = this.text().t(TextKey::ArchiveFailed);
                    this.show_error(message, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn show_error(&mut self, message: &str, cx: &mut Context<Self>) {
        self.loading = false;
        self.error = Some(message.to_string());
        cx.notify();
    }

    /// 后台分级解码并替换当前图片
    pub(super) fn open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.load_id = self.load_id.wrapping_add(1);
        let load_id = self.load_id;
        self.loading = true;
        self.error = None;
        self.sharpen = None;
        self.sharpen_failed = None;
        self.sync_folder(&path, window, cx);

        let need_px = self.need_px(window);

        // 缓存命中：连 probe 都省掉；有更清晰的已解码档就直接复用，不白解一张粗档
        if let Some(meta) = self.cache.meta(&path) {
            let max_denom = decode::pick_denom_for(&meta, Rotation::None, need_px);
            if let Some((denom, texture)) =
                self.cache.best_for(&path, Rotation::None, max_denom)
            {
                eprintln!(
                    "[SPIKE] {} hit cache 1/{denom} ({}x{})",
                    folder::file_name(&path),
                    meta.storage.0,
                    meta.storage.1
                );
                self.loading = false;
                self.view = viewer::ViewState::default();
                self.present_cached(path, meta, Rotation::None, denom, texture, cx);
                self.prefetch_neighbors(window, cx);
                return;
            }
        }

        eprintln!(
            "[SPIKE] loading {} (need {need_px}px @{}x)",
            path.display(),
            window.scale_factor()
        );
        cx.notify();

        cx.spawn(async move |this, cx| {
            // 排队期间已被更新的请求取代：直接跳过，不占后台解码线程
            if stale(&this, cx, load_id) {
                return;
            }
            let name = folder::file_name(&path);
            let result = cx
                .background_executor()
                .spawn(decode_task(path.clone(), need_px, Rotation::None, None))
                .await;

            this.update_in(cx, |this, window, cx| {
                // 已被更新的请求取代
                if this.load_id != load_id {
                    return;
                }
                this.loading = false;
                match result {
                    Ok(Decoded {
                        meta,
                        preview,
                        elapsed,
                    }) => {
                        let display = meta.display_size();
                        let denom = preview.denom;
                        let backend = preview.backend;
                        let tex_size = (preview.width(), preview.height());
                        let tex_mb = preview.byte_len() as f64 / 1_048_576.0;
                        this.view = viewer::ViewState::default();
                        this.error = None;
                        let texture = viewer::texture(preview);
                        this.present(path, meta, Rotation::None, denom, texture, window, cx);
                        // 打点在 present 之后：此时缓存已更新，读到的才是上屏真实占用
                        eprintln!(
                            "[SPIKE] {name} → {}x{} (1/{denom}) via {backend} in {} ms, texture {}x{} {tex_mb:.2} MB · {}",
                            display.0,
                            display.1,
                            elapsed.as_millis(),
                            tex_size.0,
                            tex_size.1,
                            this.mem_usage()
                        );
                        this.prefetch_neighbors(window, cx);
                    }
                    Err(message) => {
                        this.error = Some(message);
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// 上一张 / 下一张（循环）
    pub(super) fn step_image(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(folder) = self.folder.as_mut() else {
            return;
        };
        let Some(path) = folder.step(forward).map(|path| path.to_path_buf()) else {
            return;
        };
        self.open_path(path, window, cx);
    }

    /// 旋转当前图片；切图时角度会重置为 0°
    pub(super) fn rotate(&mut self, clockwise: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(current) = self.image.as_ref() else {
            return;
        };
        let path = current.path.clone();
        let rotation = current.rotation.step(clockwise);
        if rotation == current.rotation {
            return;
        }
        // 提前克隆纹理与档位，结束对 self.image 的借用（后面要可变借用 self）
        let source = Arc::clone(&current.image);
        let current_denom = current.denom;
        let Some(meta) = self.cache.meta(&path) else {
            return;
        };
        self.touch_status(cx);

        // 任何旋转都作废旧任务；连续按 R 时只保留最后一次
        self.rotate_id = self.rotate_id.wrapping_add(1);
        let rotate_id = self.rotate_id;

        // 目标角度的选档可能和当前角度不同（90° 时源宽变成源高），
        // 必须用同一个 need_px 算出的 denom 查缓存，否则来回旋转永远不命中。
        let need_px = self.need_px(window);
        let denom = decode::pick_denom_for(&meta, rotation, need_px);

        // 命中缓存：直接换纹理。不能再走 present，否则同一个 Arc 会被当成旧档位回收
        if let Some(texture) = self.cache.get(&path, rotation, denom) {
            self.image = Some(viewer::LoadedImage {
                path,
                image: texture,
                pixel_size: decode::display_size(&meta, rotation),
                denom,
                rotation,
                scalable: meta.scalable,
            });
            self.view = viewer::ViewState::default();
            cx.notify();
            return;
        }

        let name = folder::file_name(&path);

        // 即时旋转当前纹理：90° 像素重排 ~20ms，远快于重解全图 120ms
        // 走 present 让纹理纳入缓存管理（否则 atlas 无法回收）；denom 沿用当前档位，
        // 下一帧 schedule_sharpen 会按目标角度重新选档，后台补到足够清晰。
        if let Some(rotated) = viewer::rotate_texture(&source, clockwise) {
            eprintln!("[SPIKE] {name} rotated to {rotation:?} (instant texture)");
            self.view = viewer::ViewState::default();
            self.present(
                path.clone(),
                meta,
                rotation,
                current_denom,
                Arc::new(rotated),
                window,
                cx,
            );
            return;
        }

        // 纹理字节不可用（理论上不会发生）：退回原始重解路径，meta 可复用
        let load_id = self.load_id;
        let Some(meta) = self.cache.meta(&path) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            if this
                .read_with(cx, |this, _| {
                    this.rotate_id != rotate_id || this.load_id != load_id
                })
                .unwrap_or(true)
            {
                return;
            }
            let result = cx
                .background_executor()
                .spawn(decode_task(path.clone(), need_px, rotation, Some(meta)))
                .await;

            this.update_in(cx, |this, window, cx| {
                if this.rotate_id != rotate_id || this.load_id != load_id {
                    return;
                }
                let Ok(Decoded { meta, preview, .. }) = result else {
                    return;
                };
                let denom = preview.denom;
                let texture = viewer::texture(preview);
                this.view = viewer::ViewState::default();
                this.present(path, meta, rotation, denom, texture, window, cx);
            })
            .ok();
        })
        .detach();
    }

    /// 命中缓存的纹理上屏：纹理已在 cache 中且 LRU 已刷新，
    /// 绝不能再走 present（同 key 替换会把正在上屏的 Arc 当成旧纹理 drop_image）。
    fn present_cached(
        &mut self,
        path: PathBuf,
        meta: ImageMeta,
        rotation: Rotation,
        denom: u32,
        texture: Arc<RenderImage>,
        cx: &mut Context<Self>,
    ) {
        let refresh_info = self
            .photo_info
            .as_ref()
            .and_then(|state| (state.path != path).then(|| path.clone()));
        let changed = self
            .image
            .as_ref()
            .is_none_or(|current| current.path != path);
        self.image = Some(viewer::LoadedImage {
            path,
            image: texture,
            pixel_size: decode::display_size(&meta, rotation),
            denom,
            rotation,
            scalable: meta.scalable,
        });
        if changed {
            self.touch_status(cx);
        }
        if let Some(path) = refresh_info {
            self.refresh_photo_info(path, cx);
        }
        cx.notify();
    }

    /// 上屏并写入缓存：`self.image` 与缓存持有同一个 `Arc`
    ///
    /// 纹理的回收责任从这里转到缓存：淘汰时才 `drop_image`，
    /// 这正是「切回看过的图零成本」的前提。
    fn present(
        &mut self,
        path: PathBuf,
        meta: ImageMeta,
        rotation: Rotation,
        denom: u32,
        texture: Arc<RenderImage>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let refresh_info = self
            .photo_info
            .as_ref()
            .and_then(|state| (state.path != path).then(|| path.clone()));
        let dropped = self.cache.insert(
            path.clone(),
            meta,
            rotation,
            denom,
            Arc::clone(&texture),
            &path,
        );
        // 换图才重放状态徽标淡入；补清晰档位 / 旋转时不动
        let changed = self
            .image
            .as_ref()
            .is_none_or(|current| current.path != path);
        self.image = Some(viewer::LoadedImage {
            path,
            image: texture,
            pixel_size: decode::display_size(&meta, rotation),
            denom,
            rotation,
            scalable: meta.scalable,
        });
        if changed {
            // 换图后重新显示状态栏，并重置自动淡出计时
            self.touch_status(cx);
        }
        for image in dropped {
            let _ = window.drop_image(image);
        }
        if let Some(path) = refresh_info {
            self.refresh_photo_info(path, cx);
        }
        cx.notify();
    }

    /// 预取前后各一张，让连续翻页变成瞬时
    fn prefetch_neighbors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(folder) = self.folder.as_ref() else {
            return;
        };
        let need_px = self.need_px(window);
        let load_id = self.load_id;

        // 目录只有两张图时前后邻居重合，必须去重，否则同一张会被解两次
        let targets: Vec<PathBuf> = [-1isize, 1]
            .into_iter()
            .filter_map(|delta| folder.peek(delta).map(|path| path.to_path_buf()))
            .filter(|path| !self.cache.contains(path) && !self.prefetching.contains(path))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        if targets.is_empty() {
            return;
        }
        for path in &targets {
            self.prefetching.insert(path.clone());
        }

        cx.spawn(async move |this, cx| {
            for path in targets {
                // 已翻页：跳过解码，但仍进 update_in 清 prefetching 标志
                let result = if stale(&this, cx, load_id) {
                    None
                } else {
                    let started = Instant::now();
                    let decoded = cx
                        .background_executor()
                        .spawn(decode_task(path.clone(), need_px, Rotation::None, None))
                        .await;
                    Some((decoded, started))
                };

                this.update_in(cx, |this, window, cx| {
                    this.prefetching.remove(&path);
                    let Some((Ok(Decoded { meta, preview, .. }), started)) = result else {
                        return;
                    };
                    // 用户已经切到这张了：由正常路径接管，别用预取结果顶掉在用纹理
                    if this.image.as_ref().is_some_and(|image| image.path == path) {
                        return;
                    }
                    let denom = preview.denom;
                    let texture = viewer::texture(preview);
                    let keep = this
                        .image
                        .as_ref()
                        .map(|image| image.path.clone())
                        .unwrap_or_default();

                    let dropped = this.cache.insert(
                        path.clone(),
                        meta,
                        Rotation::None,
                        denom,
                        texture,
                        &keep,
                    );
                    for image in dropped {
                        let _ = window.drop_image(image);
                    }
                    eprintln!(
                        "[SPIKE] prefetched {} 1/{denom} in {} ms",
                        folder::file_name(&path),
                        started.elapsed().as_millis()
                    );
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// 当前档位不足以覆盖显示尺寸时，补一张更清晰的
    ///
    /// 触发点在 `render`：窗口缩放、全屏、模式切换、滚轮缩放都会反映到 `scale()` 上，
    /// 不必在每个入口分别调用。`sharpen` 记录正在解的档位，避免重复提交。
    pub(super) fn schedule_sharpen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(image) = self.image.as_ref() else {
            return;
        };
        let view = self.view_size(window);
        let base = image.logical_size(window.scale_factor());
        let need_px = (image.pixel_size.0 as f32 * self.view.scale(view, base))
            .round()
            .max(1.0) as u32;

        if !image.needs_sharper(need_px) {
            return;
        }
        let want = decode::pick_denom(image.pixel_size.0, need_px);
        let path = image.path.clone();
        let rotation = image.rotation;
        // 去重 key 含图片与角度：缩放中途 want 变化或换图后，旧任务不再代表当前需求
        let busy = |key: &Option<(PathBuf, Rotation, u32)>| {
            key.as_ref()
                .is_some_and(|(p, r, d)| p == &path && r == &rotation && *d == want)
        };
        if busy(&self.sharpen) || busy(&self.sharpen_failed) {
            return;
        }

        self.sharpen = Some((path.clone(), rotation, want));
        let load_id = self.load_id;

        // 翻页往返时同档可能已缓存：命中直接换纹理，省掉后台全解码
        let known_meta = self.cache.meta(&path);
        if let Some(meta) = known_meta
            && let Some(texture) = self.cache.get(&path, rotation, want)
        {
            eprintln!(
                "[SPIKE] {} sharpen hit cache 1/{want}",
                folder::file_name(&path)
            );
            // 同步完成补档，清掉占位标志；present_cached 不改 view
            self.sharpen = None;
            self.present_cached(path, meta, rotation, want, texture, cx);
            return;
        }

        eprintln!("[SPIKE] sharpening to 1/{want} (need {need_px}px)");

        cx.spawn(async move |this, cx| {
            // 排队期间已切图：清掉占位标志，让新图的补档不受阻
            if stale(&this, cx, load_id) {
                this.update(cx, |this, cx| {
                    if this.sharpen.as_ref() == Some(&(path.clone(), rotation, want)) {
                        this.sharpen = None;
                    }
                    cx.notify();
                })
                .ok();
                return;
            }
            let result = cx
                .background_executor()
                .spawn(decode_task(path.clone(), need_px, rotation, known_meta))
                .await;

            this.update_in(cx, |this, window, cx| {
                // 只有仍是当前在途补档才落地：缩放中途发起的更新档会覆盖 sharpen，
                // 此时旧档结果是过期的，既不能清标志也不能上屏（否则反向降档）。
                let key = (path.clone(), rotation, want);
                if this.sharpen.as_ref() != Some(&key) {
                    return;
                }
                this.sharpen = None;
                // 旋转后旧角度的补档结果不能覆盖新纹理
                if !this
                    .image
                    .as_ref()
                    .is_some_and(|image| image.path == path && image.rotation == rotation)
                {
                    return;
                }
                match result {
                    Ok(Decoded {
                        meta,
                        preview,
                        elapsed,
                    }) => {
                        let denom = preview.denom;
                        let tex_size = (preview.width(), preview.height());
                        let tex_mb = preview.byte_len() as f64 / 1_048_576.0;
                        let texture = viewer::texture(preview);
                        this.present(path, meta, rotation, denom, texture, window, cx);
                        eprintln!(
                            "[SPIKE] sharpened to 1/{denom} in {} ms, texture {}x{} {tex_mb:.2} MB · {}",
                            elapsed.as_millis(),
                            tex_size.0,
                            tex_size.1,
                            this.mem_usage()
                        );
                    }
                    // 补档失败保持原图，不打扰用户；记下精确档避免每帧重试
                    Err(_) => {
                        this.sharpen_failed = Some(key);
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }
}
