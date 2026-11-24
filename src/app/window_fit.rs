//! 打开图片时窗口跟随图片尺寸的纯计算逻辑。

use gpui::{Pixels, Size, px, size};

/// 计算目标内容尺寸；图片装不进当前工作区时不调整。
pub fn target_size(
    image: Size<Pixels>,
    work: Size<Pixels>,
    chrome: Pixels,
    min: Size<Pixels>,
) -> Option<Size<Pixels>> {
    let work_w: f32 = work.width.into();
    let work_h: f32 = work.height.into();
    if work_w < f32::from(min.width) || work_h < f32::from(min.height) {
        return None;
    }

    let width: f32 = image.width.into();
    let height: f32 = f32::from(image.height) + f32::from(chrome);
    if width > work_w || height > work_h {
        return None;
    }

    Some(size(
        px(width.max(f32::from(min.width)).min(work_w)),
        px(height.max(f32::from(min.height)).min(work_h)),
    ))
}

#[cfg(test)]
mod tests {
    use super::target_size;
    use gpui::{px, size};

    #[test]
    fn target_size_follows_small_image_and_skips_large_one() {
        let work = size(px(1920.0), px(1040.0));
        let min = size(px(420.0), px(320.0));

        let target = target_size(size(px(800.0), px(600.0)), work, px(32.0), min).unwrap();
        assert_eq!(target, size(px(800.0), px(632.0)));
        assert!(target_size(size(px(3000.0), px(2000.0)), work, px(32.0), min).is_none());
    }
}
