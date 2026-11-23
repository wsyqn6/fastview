//! 照片信息：文件格式、大小与常用 EXIF 的纯逻辑解析

use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use exif::{Exif, In, Rational, Reader, Tag};
use image::ImageFormat;

/// 按路径缓存的照片信息上限
pub const CACHE_LIMIT: usize = 256;

#[derive(Clone, Debug, Default)]
pub struct PhotoInfoData {
    pub format: Option<String>,
    pub size_bytes: u64,
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens: Option<String>,
    pub focal_length: Option<String>,
    pub taken_at: Option<String>,
    pub iso: Option<u32>,
    pub shutter: Option<String>,
    pub aperture: Option<String>,
    /// EXIF 不存在或解析失败；基础字段仍然可用
    pub exif_unavailable: bool,
}

#[derive(Clone, Debug)]
pub struct CachedPhotoInfo {
    pub len: u64,
    pub modified: SystemTime,
    pub data: Arc<PhotoInfoData>,
}

impl CachedPhotoInfo {
    pub fn matches(&self, len: u64, modified: SystemTime) -> bool {
        self.len == len && self.modified == modified
    }
}

/// 文件指纹：缓存命中校验用
pub fn fingerprint(path: &Path) -> Result<(u64, SystemTime), String> {
    let meta = fs::metadata(path).map_err(|err| format!("无法读取文件信息：{err}"))?;
    Ok((meta.len(), meta.modified().map_err(|err| format!("无法读取修改时间：{err}"))?))
}

/// 读取文件信息与常用 EXIF；失败只影响本张照片的信息面板
pub fn collect(path: &Path) -> Result<PhotoInfoData, String> {
    let file_meta = fs::metadata(path).map_err(|err| format!("无法读取文件信息：{err}"))?;
    let mut data = PhotoInfoData {
        format: detect_format(path),
        size_bytes: file_meta.len(),
        ..PhotoInfoData::default()
    };

    let file = File::open(path).map_err(|err| format!("无法打开文件：{err}"))?;
    match Reader::new().read_from_container(&mut BufReader::new(&file)) {
        Ok(exif) => fill_exif(&mut data, &exif),
        Err(_) => data.exif_unavailable = true,
    }
    Ok(data)
}

fn detect_format(path: &Path) -> Option<String> {
    use image::ImageReader;

    let detected = ImageReader::open(path)
        .ok()
        .and_then(|reader| reader.with_guessed_format().ok())
        .and_then(|reader| reader.format())
        .and_then(format_name);
    detected.or_else(|| extension_format(path))
}

fn format_name(format: ImageFormat) -> Option<String> {
    let name = match format {
        ImageFormat::Jpeg => "JPEG",
        ImageFormat::Png => "PNG",
        ImageFormat::WebP => "WebP",
        ImageFormat::Gif => "GIF",
        ImageFormat::Bmp => "BMP",
        ImageFormat::Tiff => "TIFF",
        _ => return None,
    };
    Some(name.to_string())
}

fn extension_format(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_string_lossy().to_ascii_uppercase();
    match extension.as_str() {
        "JPG" | "JPEG" => Some("JPEG".to_string()),
        "PNG" | "WEBP" | "GIF" | "BMP" | "TIF" | "TIFF" => Some(extension),
        _ => None,
    }
}

fn fill_exif(data: &mut PhotoInfoData, exif: &Exif) {
    data.make = field_text(exif, Tag::Make);
    data.model = field_text(exif, Tag::Model);
    data.lens = field_text(exif, Tag::LensModel).or_else(|| field_text(exif, Tag::LensMake));
    data.focal_length = field_rational(exif, Tag::FocalLength).and_then(format_focal);
    data.taken_at = field_text(exif, Tag::DateTimeOriginal)
        .or_else(|| field_text(exif, Tag::DateTime))
        .and_then(normalize_date);
    data.iso = field_uint(exif, Tag::PhotographicSensitivity)
        .or_else(|| field_uint(exif, Tag::ISOSpeed));
    data.shutter = field_rational(exif, Tag::ExposureTime).and_then(format_shutter);
    data.aperture = field_rational(exif, Tag::FNumber).and_then(format_aperture);
}

fn field_text(exif: &Exif, tag: Tag) -> Option<String> {
    let text = exif
        .get_field(tag, In::PRIMARY)?
        .display_value()
        .to_string();
    let text = text.trim();
    (!text.is_empty() && text != "-").then(|| text.to_string())
}

fn field_rational(exif: &Exif, tag: Tag) -> Option<Rational> {
    match exif.get_field(tag, In::PRIMARY)?.value {
        exif::Value::Rational(ref values) => values.first().copied(),
        _ => None,
    }
}

fn field_uint(exif: &Exif, tag: Tag) -> Option<u32> {
    exif.get_field(tag, In::PRIMARY)?
        .value
        .get_uint(0)
        .filter(|value| *value > 0)
}

/// `2024:01:02 03:04:05` → `2024-01-02 03:04:05`
///
/// 只改日期段的前两个冒号；纯时间值（如 `03:04:05`）原样保留
fn normalize_date(value: String) -> Option<String> {
    let bytes = value.as_bytes();
    let is_date = bytes.len() >= 10
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b':'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b':'
        && bytes[8..10].iter().all(u8::is_ascii_digit);

    let out = if is_date {
        let mut chars: Vec<char> = value.chars().collect();
        chars[4] = '-';
        chars[7] = '-';
        chars.into_iter().collect()
    } else {
        value
    };
    (!out.trim().is_empty()).then_some(out)
}

/// 快门：`1/250 s` 或 `2.5 s`
pub fn format_shutter(value: Rational) -> Option<String> {
    if value.num == 0 || value.denom == 0 {
        return None;
    }
    let seconds = value.num as f64 / value.denom as f64;
    let text = if seconds >= 1.0 {
        format!("{} s", trim_float(seconds))
    } else {
        format!("1/{} s", trim_float(1.0 / seconds))
    };
    Some(text)
}

pub fn format_aperture(value: Rational) -> Option<String> {
    nonzero_float(value).map(|value| format!("f/{}", trim_float(value)))
}

pub fn format_focal(value: Rational) -> Option<String> {
    nonzero_float(value).map(|value| format!("{} mm", trim_float(value)))
}

fn nonzero_float(value: Rational) -> Option<f64> {
    if value.num == 0 || value.denom == 0 {
        None
    } else {
        Some(value.num as f64 / value.denom as f64)
    }
}

fn trim_float(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{:.1} KB", kb);
    }
    format!("{:.1} MB", kb / 1024.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rational(num: u32, denom: u32) -> Rational {
        Rational { num, denom }
    }

    #[test]
    fn formats_shutter_and_aperture() {
        assert_eq!(format_shutter(rational(1, 250)).as_deref(), Some("1/250 s"));
        assert_eq!(format_shutter(rational(5, 2)).as_deref(), Some("2.5 s"));
        assert_eq!(format_aperture(rational(28, 10)).as_deref(), Some("f/2.8"));
        assert_eq!(format_focal(rational(350, 10)).as_deref(), Some("35 mm"));
    }

    #[test]
    fn formats_size_and_date() {
        assert_eq!(format_size(800), "800 B");
        assert_eq!(format_size(2048), "2.0 KB");
        assert_eq!(format_size(3 * 1024 * 1024), "3.0 MB");
        assert_eq!(
            normalize_date("2024:01:02 03:04:05".to_string()).as_deref(),
            Some("2024-01-02 03:04:05")
        );
    }

    #[test]
    fn normalize_date_keeps_time_only_value() {
        assert_eq!(
            normalize_date("03:04:05".to_string()).as_deref(),
            Some("03:04:05")
        );
        assert_eq!(
            normalize_date("2024:01:02".to_string()).as_deref(),
            Some("2024-01-02")
        );
        assert_eq!(normalize_date("  ".to_string()), None);
    }

    #[test]
    fn cache_matches_file_fingerprint() {
        let modified = std::time::UNIX_EPOCH;
        let cached = CachedPhotoInfo {
            len: 10,
            modified,
            data: Arc::new(PhotoInfoData::default()),
        };

        assert!(cached.matches(10, modified));
        assert!(!cached.matches(11, modified));
    }
}
