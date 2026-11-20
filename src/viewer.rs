//! 图片解码与查看状态
//!
//! 解码走 `crate::image` 的分级管道：按当前需要的设备像素选档，不整图解码。
//! 分块渲染、缓存、翻页后续再补。

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{Bounds, Pixels, Point, RenderImage, Size, point, px, size};

use crate::decode::{Preview, Rotation, pick_denom};

/// 缩放上下限（逻辑像素 / 图片逻辑像素）
const MIN_ZOOM: f32 = 0.02;
const MAX_ZOOM: f32 = 32.0;
/// 平移时至少保留这么多图片可见，避免拖丢
const KEEP_VISIBLE: f32 = 24.0;

/// 已上屏的图片
///
/// 纹理可能是低分辨率预览，`pixel_size` 始终是原始显示尺寸，
/// 因此布局与缩放百分比不受预览档位影响。
pub struct LoadedImage {
    pub path: PathBuf,
    pub image: Arc<RenderImage>,
    /// 显示尺寸（已应用 EXIF 方向与用户旋转，设备像素）
    pub pixel_size: (u32, u32),
    /// 当前纹理相对原始尺寸的分频（1/denom）
    pub denom: u32,
    /// 当前纹理已应用的用户旋转（切图时重置）
    pub rotation: Rotation,
}

impl LoadedImage {
    /// 图片的逻辑尺寸（扣掉 HiDPI 缩放比）
    pub fn logical_size(&self, scale_factor: f32) -> Size<Pixels> {
        size(
            px(self.pixel_size.0 as f32 / scale_factor),
            px(self.pixel_size.1 as f32 / scale_factor),
        )
    }

    pub fn file_name(&self) -> String {
        crate::folder::file_name(&self.path)
    }

    /// 当前纹理是否已不足以覆盖 `need_px`（需要补更清晰的一档）
    pub fn needs_sharper(&self, need_px: u32) -> bool {
        pick_denom(self.pixel_size.0, need_px) < self.denom
    }
}

/// 把解码结果转成可上屏、可缓存的纹理
pub fn texture(preview: Preview) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(preview.buffer)]))
}

/// 缩放方式
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    /// 适应窗口，窗口变化时自动重算
    Fit,
    /// 100%（1 图片像素 = 1 设备像素）
    Original,
    /// 填满窗口
    Fill,
    /// 自由缩放（滚轮 / 快捷键）
    Manual,
}

/// 查看状态：缩放与平移
pub struct ViewState {
    pub mode: ViewMode,
    /// 仅 `Manual` 模式使用；其余模式由窗口与图片尺寸推导
    pub zoom: f32,
    /// 图片中心相对视口中心的偏移（逻辑像素）
    pub offset: Point<Pixels>,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            mode: ViewMode::Fit,
            zoom: 1.0,
            offset: point(px(0.0), px(0.0)),
        }
    }
}

impl ViewState {
    /// 当前生效的缩放比
    pub fn scale(&self, view: Size<Pixels>, base: Size<Pixels>) -> f32 {
        let base_w: f32 = base.width.into();
        let base_h: f32 = base.height.into();
        if base_w <= 0.0 || base_h <= 0.0 {
            return 1.0;
        }

        let view_w: f32 = view.width.into();
        let view_h: f32 = view.height.into();
        let fit = (view_w / base_w).min(view_h / base_h);

        match self.mode {
            ViewMode::Fit => fit,
            ViewMode::Fill => (view_w / base_w).max(view_h / base_h),
            ViewMode::Original => 1.0,
            ViewMode::Manual => self.zoom,
        }
    }

    /// 切换缩放方式并复位平移
    pub fn set_mode(&mut self, mode: ViewMode, view: Size<Pixels>, base: Size<Pixels>) {
        self.mode = mode;
        self.offset = point(px(0.0), px(0.0));
        if mode == ViewMode::Manual {
            self.zoom = self.scale(view, base);
        }
    }

    /// 以光标为锚点缩放
    pub fn zoom_at(
        &mut self,
        factor: f32,
        cursor: Point<Pixels>,
        view: Size<Pixels>,
        base: Size<Pixels>,
    ) {
        let old = self.scale(view, base);
        let new = (old * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        if (new - old).abs() < f32::EPSILON {
            return;
        }

        let view_w: f32 = view.width.into();
        let view_h: f32 = view.height.into();
        let cursor_x: f32 = cursor.x.into();
        let cursor_y: f32 = cursor.y.into();
        let cx = cursor_x - view_w / 2.0;
        let cy = cursor_y - view_h / 2.0;
        let ox: f32 = self.offset.x.into();
        let oy: f32 = self.offset.y.into();

        // 光标下的图片坐标在缩放前后保持不动
        let img_x = (cx - ox) / old;
        let img_y = (cy - oy) / old;

        self.zoom = new;
        self.mode = ViewMode::Manual;
        self.offset = point(px(cx - img_x * new), px(cy - img_y * new));
        self.clamp_offset(view, base);
    }

    /// 把图片中的某个点移到视口中心（minimap 点击 / 拖动用）
    pub fn center_on(&mut self, target: Point<Pixels>, view: Size<Pixels>, base: Size<Pixels>) {
        let scale = self.scale(view, base);
        let base_w: f32 = base.width.into();
        let base_h: f32 = base.height.into();
        let x: f32 = target.x.into();
        let y: f32 = target.y.into();

        // image_bounds 原点 = view/2 - base*scale/2 + offset，
        // 让 target 落在视口中心时对 offset 求解即可。
        self.offset = point(
            px((base_w / 2.0 - x) * scale),
            px((base_h / 2.0 - y) * scale),
        );
        self.clamp_offset(view, base);
    }

    /// 平移
    pub fn pan(&mut self, delta: Point<Pixels>, view: Size<Pixels>, base: Size<Pixels>) {
        self.offset = point(self.offset.x + delta.x, self.offset.y + delta.y);
        self.clamp_offset(view, base);
    }

    /// 限制平移范围，保证图片不会完全拖出视口
    fn clamp_offset(&mut self, view: Size<Pixels>, base: Size<Pixels>) {
        let zoom = self.scale(view, base);
        let view_w: f32 = view.width.into();
        let view_h: f32 = view.height.into();
        let img_w: f32 = base.width.into();
        let img_h: f32 = base.height.into();
        let img_w = img_w * zoom;
        let img_h = img_h * zoom;

        let max_x = ((img_w - KEEP_VISIBLE) / 2.0).max(0.0);
        let max_y = ((img_h - KEEP_VISIBLE) / 2.0).max(0.0);
        // 图片比视口小时不许拖走，保持居中
        let max_x = if img_w <= view_w { 0.0 } else { max_x };
        let max_y = if img_h <= view_h { 0.0 } else { max_y };

        let x: f32 = self.offset.x.into();
        let y: f32 = self.offset.y.into();
        self.offset = point(
            px(x.clamp(-max_x, max_x)),
            px(y.clamp(-max_y, max_y)),
        );
    }

    /// 图片在视口内的绘制区域（视口坐标）
    pub fn image_bounds(&self, view: Size<Pixels>, base: Size<Pixels>) -> Bounds<Pixels> {
        let zoom = self.scale(view, base);
        let img_w: f32 = base.width.into();
        let img_h: f32 = base.height.into();
        let img_w = img_w * zoom;
        let img_h = img_h * zoom;
        let view_w: f32 = view.width.into();
        let view_h: f32 = view.height.into();
        let ox: f32 = self.offset.x.into();
        let oy: f32 = self.offset.y.into();

        Bounds::new(
            point(
                px((view_w - img_w) / 2.0 + ox),
                px((view_h - img_h) / 2.0 + oy),
            ),
            size(px(img_w), px(img_h)),
        )
    }

    /// 显示百分比（100% = 1 图片像素对 1 设备像素）
    pub fn percent(&self, view: Size<Pixels>, base: Size<Pixels>) -> f32 {
        self.scale(view, base) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::{ViewMode, ViewState, point, px, size};

    fn viewport() -> gpui::Size<gpui::Pixels> {
        size(px(400.0), px(400.0))
    }

    fn large_image() -> gpui::Size<gpui::Pixels> {
        size(px(1000.0), px(1000.0))
    }

    fn manual_zoom() -> ViewState {
        ViewState {
            mode: ViewMode::Manual,
            zoom: 2.0,
            ..ViewState::default()
        }
    }

    #[test]
    fn center_on_image_center_clears_offset() {
        let mut view = manual_zoom();
        view.center_on(point(px(500.0), px(500.0)), viewport(), large_image());
        let offset: f32 = view.offset.x.into();
        assert_eq!(offset, 0.0);
        let offset: f32 = view.offset.y.into();
        assert_eq!(offset, 0.0);
    }

    #[test]
    fn center_on_corner_is_clamped() {
        let mut view = manual_zoom();
        view.center_on(point(px(1000.0), px(1000.0)), viewport(), large_image());
        let x: f32 = view.offset.x.into();
        let y: f32 = view.offset.y.into();
        // 原始计算为 -1000，clamp 到 -988（KEEP_VISIBLE=24）
        assert_eq!((x, y), (-988.0, -988.0));
    }

    #[test]
    fn center_on_small_image_stays_centered() {
        let mut view = ViewState::default();
        let small = size(px(100.0), px(100.0));
        view.center_on(point(px(50.0), px(50.0)), viewport(), small);
        let x: f32 = view.offset.x.into();
        let y: f32 = view.offset.y.into();
        assert_eq!((x, y), (0.0, 0.0));
    }
}
