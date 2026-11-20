//! Windows 系统缩略图（thumbcache）
//!
//! Explorer 早已把常见图片的缩略图写进 `thumbcache_*.db`，命中时 `GetImage`
//! 只是拷一块位图，CPU 几乎为零。首次未命中会交给 shell 现场解码并写回缓存，
//! 所以这里允许生成（不带 `SIIGBF_THUMBNAILONLY`）。
//!
//! 实测：命中 3~10 ms，首次生成 50~165 ms。

use std::path::Path;

use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits, GetObjectW,
    HGDIOBJ, HBITMAP, ReleaseDC,
};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SIIGBF_BIGGERSIZEOK, SHCreateItemFromParsingName,
};
use windows::core::HSTRING;

/// 一张缩略图（BGRA，自上而下）
pub struct Thumbnail {
    pub bgra: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// 取系统缩略图，长边不超过 `edge`
pub fn thumbnail(path: &Path, edge: u32) -> Result<Thumbnail, String> {
    crate::com::ensure()?;

    let wide = HSTRING::from(path.to_string_lossy().as_ref());
    let factory: IShellItemImageFactory =
        unsafe { SHCreateItemFromParsingName(&wide, None) }.map_err(shell_err)?;

    let edge = edge.clamp(16, 1024) as i32;
    let bitmap = unsafe {
        factory.GetImage(
            SIZE { cx: edge, cy: edge },
            // 大图更好时允许 shell 给更大的尺寸，避免二次放大
            SIIGBF_BIGGERSIZEOK,
        )
    }
    .map_err(shell_err)?;

    let (width, height) = unsafe { bitmap_size(bitmap) };
    if width <= 0 || height <= 0 {
        unsafe { let _ = DeleteObject(HGDIOBJ(bitmap.0)); }
        return Err("缩略图尺寸无效".into());
    }

    let converted = unsafe { to_bgra(bitmap, width, height) };
    unsafe { let _ = DeleteObject(HGDIOBJ(bitmap.0)); }

    let bgra = converted.ok_or_else(|| "缩略图位图转换失败".to_string())?;
    Ok(Thumbnail {
        bgra,
        width: width as u32,
        height: height as u32,
    })
}

fn shell_err(error: windows::core::Error) -> String {
    format!("shell 缩略图：{error}")
}

unsafe fn bitmap_size(bitmap: HBITMAP) -> (i32, i32) {
    let mut info = BITMAP::default();
    let read = unsafe {
        GetObjectW(
            HGDIOBJ(bitmap.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut info as *mut _ as *mut _),
        )
    };
    if read > 0 {
        (info.bmWidth, info.bmHeight)
    } else {
        (0, 0)
    }
}

/// HBITMAP -> BGRA（gpui 的 `RenderImage` 正好吃 BGRA）
unsafe fn to_bgra(bitmap: HBITMAP, width: i32, height: i32) -> Option<Vec<u8>> {
    let mut info = BITMAPINFO::default();
    info.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        biHeight: -height, // 负高度 = top-down
        biPlanes: 1,
        biBitCount: 32,
        biCompression: 0, // BI_RGB
        ..Default::default()
    };

    let mut buffer = vec![0u8; (width as usize) * (height as usize) * 4];
    unsafe {
        let device = GetDC(None);
        let lines = GetDIBits(
            device,
            bitmap,
            0,
            height as u32,
            Some(buffer.as_mut_ptr() as *mut _),
            &mut info,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, device);
        (lines > 0).then_some(buffer)
    }
}
