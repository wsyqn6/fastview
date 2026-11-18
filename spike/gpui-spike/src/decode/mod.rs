//! 图像管道：探测 → 选档 → 缩放解码 → 方向归一
//!
//! 设计要点（详见 AGENTS.md「图像管道」）：
//! - 只按「当前需要的设备像素」解码，不整图解码；
//! - WIC 负责缩放解码，`image` 兜底；
//! - EXIF 方向在解码后统一应用，避免各后端行为不一致。

mod fallback;
#[cfg(windows)]
mod wic;

use std::path::Path;

use image::metadata::Orientation;

/// WIC 支持的最大分频；JPEG 的 DCT 缩放只能是 1/2ⁿ
const MAX_DENOM: u32 = 8;

/// 图片元信息（尺寸为「存储尺寸」，未应用方向）
#[derive(Clone, Copy, Debug)]
pub struct ImageMeta {
    pub storage: (u32, u32),
    pub orientation: Orientation,
}

impl ImageMeta {
    /// 应用方向后的显示尺寸
    pub fn display_size(&self) -> (u32, u32) {
        if swaps_axes(self.orientation) {
            (self.storage.1, self.storage.0)
        } else {
            self.storage
        }
    }
}

/// 一张按需解码的预览图（BGRA，自上而下）
pub struct Preview {
    pub buffer: image::RgbaImage,
    /// 相对原始尺寸的分频（1/denom），用于判断是否需要补更清晰的一档
    pub denom: u32,
    pub backend: &'static str,
}

impl Preview {
    pub fn width(&self) -> u32 {
        self.buffer.width()
    }

    pub fn height(&self) -> u32 {
        self.buffer.height()
    }

    pub fn byte_len(&self) -> usize {
        self.buffer.as_raw().len()
    }
}

/// 只读文件头拿尺寸与方向，耗时在毫秒级
pub fn probe(path: &Path) -> Result<ImageMeta, String> {
    use image::ImageDecoder;

    let reader = image::ImageReader::open(path)
        .map_err(|err| format!("无法打开文件：{err}"))?
        .with_guessed_format()
        .map_err(|err| format!("无法识别格式：{err}"))?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|err| format!("解码器初始化失败：{err}"))?;

    let storage = decoder.dimensions();
    if storage.0 == 0 || storage.1 == 0 {
        return Err("图片尺寸为 0".into());
    }
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    Ok(ImageMeta {
        storage,
        orientation,
    })
}

/// 选出仍能覆盖 `need_px` 的最大分频（分频越大解码越快）
pub fn pick_denom(width: u32, need_px: u32) -> u32 {
    let need = need_px.max(1);
    let mut denom = MAX_DENOM;
    while denom > 1 {
        if width / denom >= need {
            return denom;
        }
        denom /= 2;
    }
    1
}

/// 按目标宽度解码一张预览
///
/// `need_px` 是希望覆盖的设备像素宽度；实际分辨率会吸附到 WIC 支持的分频。
pub fn load_preview(path: &Path, meta: &ImageMeta, need_px: u32) -> Result<Preview, String> {
    let (sw, sh) = meta.storage;
    let denom = pick_denom(sw, need_px);
    let tw = (sw / denom).max(1);
    let th = (sh / denom).max(1);

    #[cfg(windows)]
    let wic_result = wic::decode_scaled(path, tw, th);
    #[cfg(not(windows))]
    let wic_result: Result<(Vec<u8>, u32, u32), String> = Err("WIC 仅 Windows 可用".into());

    let (bgra, width, height, backend) = match wic_result {
        Ok((data, w, h)) => (data, w, h, "wic"),
        Err(_) => {
            let (data, w, h) = fallback::decode_scaled(path, tw, th)?;
            (data, w, h, "image")
        }
    };

    let buffer = image::RgbaImage::from_raw(width, height, bgra)
        .ok_or_else(|| "像素缓冲尺寸不匹配".to_string())?;
    Ok(Preview {
        buffer: normalize(buffer, meta.orientation),
        denom,
        backend,
    })
}

fn swaps_axes(orientation: Orientation) -> bool {
    matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    )
}

/// 按 EXIF 方向重排像素（BGRA 与 RGBA 通道顺序无关，可直接借用 imageops）
///
/// 5/7 的顺序与 `image` 的 `apply_orientation` 保持一致：**先旋转，再水平翻转**。
fn normalize(buffer: image::RgbaImage, orientation: Orientation) -> image::RgbaImage {
    use image::imageops;

    match orientation {
        Orientation::NoTransforms => buffer,
        Orientation::FlipHorizontal => imageops::flip_horizontal(&buffer),
        Orientation::Rotate180 => imageops::rotate180(&buffer),
        Orientation::FlipVertical => imageops::flip_vertical(&buffer),
        Orientation::Rotate90 => imageops::rotate90(&buffer),
        Orientation::Rotate270 => imageops::rotate270(&buffer),
        Orientation::Rotate90FlipH => imageops::flip_horizontal(&imageops::rotate90(&buffer)),
        Orientation::Rotate270FlipH => imageops::flip_horizontal(&imageops::rotate270(&buffer)),
    }
}
