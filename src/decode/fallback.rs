//! `image` 兜底后端：WIC 不支持或解码失败时使用
//!
//! 代价是整图解码，只用在不支持的格式上。

use std::path::Path;

use image::GenericImageView;
use image::imageops::FilterType;

/// 解码并缩放到目标尺寸，返回 BGRA 像素
pub fn decode_scaled(path: &Path, tw: u32, th: u32) -> Result<(Vec<u8>, u32, u32), String> {
    let img = image::ImageReader::open(path)
        .map_err(|err| format!("无法打开文件：{err}"))?
        .with_guessed_format()
        .map_err(|err| format!("无法识别格式：{err}"))?
        .decode()
        .map_err(|err| format!("解码失败：{err}"))?;

    // 目标尺寸已由调用方按比例算好，这里直接拉伸到精确尺寸
    let resized = img.resize_exact(tw.max(1), th.max(1), FilterType::Triangle);
    let (width, height) = resized.dimensions();
    let mut rgba = resized.into_rgba8().into_raw();

    // gpui 的 RenderImage 吃 BGRA
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Ok((rgba, width, height))
}
