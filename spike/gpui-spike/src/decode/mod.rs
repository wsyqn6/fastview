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

/// 用户旋转（顺时针），在 EXIF 方向归一之后应用
///
/// 与 EXIF 分开：EXIF 是图片自带属性，用户旋转只是当前查看状态，
/// 因此旋转角要进缓存键，同一张图的不同角度各存一份纹理。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Rotation {
    #[default]
    None,
    Cw90,
    Cw180,
    Cw270,
}

impl Rotation {
    /// 是否交换宽高
    pub fn swaps_axes(self) -> bool {
        matches!(self, Self::Cw90 | Self::Cw270)
    }

    /// 一步旋转；`clockwise` 为 false 时逆时针
    pub fn step(self, clockwise: bool) -> Self {
        let turns = match self {
            Self::None => 0,
            Self::Cw90 => 1,
            Self::Cw180 => 2,
            Self::Cw270 => 3,
        };
        let turns = if clockwise { turns + 1 } else { turns + 3 } % 4;
        match turns {
            1 => Self::Cw90,
            2 => Self::Cw180,
            3 => Self::Cw270,
            _ => Self::None,
        }
    }
}

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

/// 应用 EXIF 方向与用户旋转后的显示尺寸
pub fn display_size(meta: &ImageMeta, rotation: Rotation) -> (u32, u32) {
    let (width, height) = meta.display_size();
    if rotation.swaps_axes() {
        (height, width)
    } else {
        (width, height)
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
///
/// 允许 5% 的插值放大：相邻档位的像素量差 4 倍，为了 0.4% 的差距把整图
/// 解一遍不划算（实测 1928px 需求会从 1/2 的 7.9 MB 跳到 1/1 的 31.6 MB）。
pub fn pick_denom(width: u32, need_px: u32) -> u32 {
    const UPSAMPLE_TOLERANCE: f32 = 1.05;

    let need = need_px.max(1) as f32;
    let mut denom = MAX_DENOM;
    while denom > 1 {
        if (width / denom) as f32 * UPSAMPLE_TOLERANCE >= need {
            return denom;
        }
        denom /= 2;
    }
    1
}

/// 按显示宽度对应的源尺寸选档（EXIF 与用户旋转都可能交换宽高）
pub fn pick_denom_for(meta: &ImageMeta, rotation: Rotation, need_px: u32) -> u32 {
    let swaps = swaps_axes(meta.orientation) ^ rotation.swaps_axes();
    pick_denom(
        if swaps { meta.storage.1 } else { meta.storage.0 },
        need_px,
    )
}

/// 按目标宽度解码一张预览
///
/// `need_px` 是希望覆盖的设备像素宽度；实际分辨率会吸附到 WIC 支持的分频。
pub fn load_preview(
    path: &Path,
    meta: &ImageMeta,
    need_px: u32,
    rotation: Rotation,
) -> Result<Preview, String> {
    let (sw, sh) = meta.storage;
    let denom = pick_denom_for(meta, rotation, need_px);
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
    let buffer = normalize(buffer, meta.orientation);
    // 用户旋转在 EXIF 归一之后做；imageops 的 rotate90 是顺时针
    let buffer = match rotation {
        Rotation::None => buffer,
        Rotation::Cw90 => image::imageops::rotate90(&buffer),
        Rotation::Cw180 => image::imageops::rotate180(&buffer),
        Rotation::Cw270 => image::imageops::rotate270(&buffer),
    };
    Ok(Preview {
        buffer,
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

#[cfg(test)]
mod tests {
    use super::{ImageMeta, Rotation, pick_denom, pick_denom_for};

    #[test]
    fn picks_largest_denom_covering_viewport() {
        assert_eq!(pick_denom(8000, 1440), 4);
        assert_eq!(pick_denom(3840, 1440), 2);
        assert_eq!(pick_denom(4000, 1440), 2);
    }

    #[test]
    fn tolerance_avoids_denom_jump() {
        // 实测：1928px 需求只比 1/2 档的 1920 多 8px，不值得为此解全图
        assert_eq!(pick_denom(3840, 1928), 2);
        // 超出 5% 容差则必须升档
        assert_eq!(pick_denom(3840, 2100), 1);
    }

    #[test]
    fn full_size_when_zoomed_in() {
        assert_eq!(pick_denom(3840, 3840), 1);
        assert_eq!(pick_denom(3840, 9999), 1);
    }

    #[test]
    fn rotation_cycles_both_ways() {
        assert_eq!(Rotation::None.step(true), Rotation::Cw90);
        assert_eq!(Rotation::Cw90.step(true), Rotation::Cw180);
        assert_eq!(Rotation::Cw180.step(true), Rotation::Cw270);
        assert_eq!(Rotation::Cw270.step(true), Rotation::None);
        assert_eq!(Rotation::None.step(false), Rotation::Cw270);
        assert_eq!(Rotation::Cw270.step(false), Rotation::Cw180);
    }

    #[test]
    fn quarter_turn_swaps_axes() {
        assert!(!Rotation::None.swaps_axes());
        assert!(Rotation::Cw90.swaps_axes());
        assert!(!Rotation::Cw180.swaps_axes());
        assert!(Rotation::Cw270.swaps_axes());
    }

    #[test]
    fn quarter_turn_picks_denom_by_source_height() {
        let meta = ImageMeta {
            storage: (4000, 2000),
            orientation: image::metadata::Orientation::NoTransforms,
        };
        assert_eq!(pick_denom_for(&meta, Rotation::None, 1000), 4);
        assert_eq!(pick_denom_for(&meta, Rotation::Cw90, 1000), 2);
    }
}
