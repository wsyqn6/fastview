//! WIC / Windows 缩略图缓存探针（无窗口 CLI）
//!
//! 为「分级预览 + 真按需解码」方案量化几个未知数：
//!   1. WIC 的 DCT 缩放解码（1/8、1/4、1/2、1/1）端到端耗时
//!   2. WIC 的 EXIF 内嵌缩略图是否存在、多大、多快
//!   3. `IShellItemImageFactory` 命中 Windows thumbcache 的耗时
//!   4. `image` crate 全解码基线（对照）
//!
//! 用法：`cargo run --release -- <图片路径>`

use std::time::Instant;

use windows::core::{Interface, Result, HSTRING};
use windows::Win32::Foundation::{GENERIC_READ, SIZE};
use windows::Win32::Graphics::Gdi::{
    BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits, GetObjectW,
    HGDIOBJ, ReleaseDC,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory2, GUID_WICPixelFormat32bppBGRA, IWICBitmapSource,
    IWICBitmapSourceTransform, IWICImagingFactory, WICBitmapTransformRotate0,
    WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, IBindCtx,
};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SIIGBF_BIGGERSIZEOK, SIIGBF_THUMBNAILONLY,
    SHCreateItemFromParsingName,
};

fn main() -> Result<()> {
    let path = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("用法: cargo run --release -- <图片路径>");
            return Ok(());
        }
    };

    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };

    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    println!("== {} ({:.2} MB) ==", path, size as f64 / 1_048_576.0);

    let wide = HSTRING::from(path.as_str());
    let factory: IWICImagingFactory =
        unsafe { CoCreateInstance(&CLSID_WICImagingFactory2, None, CLSCTX_INPROC_SERVER) }?;

    let (w, h) = wic_size(&factory, &wide)?;
    println!("\nWIC 尺寸: {w}x{h}  (注意: 存储尺寸, 未应用 EXIF 方向)");

    probe_embedded_thumbnail(&factory, &wide);
    probe_scaled_decode(&factory, &wide, w, h);
    probe_shell(&wide);
    probe_image_crate(&path);

    Ok(())
}

fn wic_size(factory: &IWICImagingFactory, path: &HSTRING) -> Result<(u32, u32)> {
    unsafe {
        let decoder =
            factory.CreateDecoderFromFilename(path, None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)?;
        let frame = decoder.GetFrame(0)?;
        let (mut w, mut h) = (0u32, 0u32);
        frame.GetSize(&mut w, &mut h)?;
        Ok((w, h))
    }
}

/// EXIF 内嵌缩略图：L0' 兜底方案的可行性
fn probe_embedded_thumbnail(factory: &IWICImagingFactory, path: &HSTRING) {
    println!("\n-- EXIF 内嵌缩略图 --");
    unsafe {
        let open = || -> Result<IWICBitmapSource> {
            let decoder = factory.CreateDecoderFromFilename(
                path,
                None,
                GENERIC_READ,
                WICDecodeMetadataCacheOnDemand,
            )?;
            decoder.GetFrame(0)?.GetThumbnail()
        };

        let started = Instant::now();
        let thumb = match open() {
            Ok(t) => t,
            Err(err) => {
                println!("  无 ({:#x})", err.code().0);
                return;
            }
        };
        let mut w = 0u32;
        let mut h = 0u32;
        if thumb.GetSize(&mut w, &mut h).is_err() {
            println!("  GetSize 失败");
            return;
        }
        let stride = w * 4;
        let mut buf = vec![0u8; (stride * h) as usize];
        let ok = thumb
            .CopyPixels(std::ptr::null(), stride, &mut buf)
            .is_ok();
        println!(
            "  {w}x{h} 共 {} ms, CopyPixels={}, 有效={}",
            started.elapsed().as_millis(),
            ok,
            ok && buf.iter().any(|&b| b != 0)
        );
    }
}

/// DCT 缩放解码：每次测量都重新打开 decoder，代表「冷启动看到第一帧」的真实成本
fn probe_scaled_decode(factory: &IWICImagingFactory, path: &HSTRING, w: u32, h: u32) {
    println!("\n-- WIC DCT 缩放解码 (端到端, 3 次取最优) --");
    println!("  {:<6} {:<14} {:>8} {:>10} {:>6}", "比例", "输出尺寸", "耗时", "缓冲", "有效");

    for denom in [8u32, 4, 2, 1] {
        let mut best_us = u128::MAX;
        let mut out = (0u32, 0u32);
        let mut bytes = 0usize;
        let mut valid = false;
        let mut failed = None;

        for _ in 0..3 {
            let started = Instant::now();
            match scaled_decode(factory, path, w, h, denom) {
                Ok((dw, dh, buf)) => {
                    best_us = best_us.min(started.elapsed().as_micros());
                    out = (dw, dh);
                    bytes = buf.len();
                    valid |= buf.iter().any(|&b| b != 0);
                }
                Err(err) => {
                    failed = Some(err);
                    break;
                }
            }
        }

        let label = format!("1/{denom}");
        match failed {
            Some(err) => println!("  {label:<6} 失败 {:#x}", err.code().0),
            None => println!(
                "  {label:<6} {:<14} {:>7.1}ms {:>8.2}MB {:>6}",
                format!("{}x{}", out.0, out.1),
                best_us as f64 / 1000.0,
                bytes as f64 / 1_048_576.0,
                valid
            ),
        }
    }
}

fn scaled_decode(
    factory: &IWICImagingFactory,
    path: &HSTRING,
    w: u32,
    h: u32,
    denom: u32,
) -> Result<(u32, u32, Vec<u8>)> {
    unsafe {
        let decoder =
            factory.CreateDecoderFromFilename(path, None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)?;
        let frame = decoder.GetFrame(0)?;
        // JPEG 的 frame 实现 IWICBitmapSourceTransform，支持 DCT 缩放
        let transform: IWICBitmapSourceTransform = frame.cast()?;

        // 目标尺寸必须吸附到 codec 支持的比例，否则会退化为全解码
        let mut tw = (w / denom).max(1);
        let mut th = (h / denom).max(1);
        let _ = transform.GetClosestSize(&mut tw, &mut th);

        let stride = tw * 4;
        let mut buf = vec![0u8; (stride * th) as usize];
        transform.CopyPixels(
            std::ptr::null(),
            tw,
            th,
            &GUID_WICPixelFormat32bppBGRA,
            WICBitmapTransformRotate0,
            stride,
            &mut buf,
        )?;
        Ok((tw, th, buf))
    }
}

/// Windows 缩略图缓存：命中时 O(1) 拷贝，未命中才现场解码
fn probe_shell(path: &HSTRING) {
    println!("\n-- Windows 缩略图缓存 (IShellItemImageFactory) --");
    unsafe {
        let started = Instant::now();
        let item: Result<IShellItemImageFactory> = SHCreateItemFromParsingName(path, None::<&IBindCtx>);
        let item = match item {
            Ok(item) => item,
            Err(err) => {
                println!("  创建失败 {:#x}", err.code().0);
                return;
            }
        };
        println!(
            "  SHCreateItemFromParsingName: {} ms",
            started.elapsed().as_millis()
        );

        let cases = [
            ("缓存 256 (THUMBNAILONLY)", 256, SIIGBF_THUMBNAILONLY | SIIGBF_BIGGERSIZEOK),
            ("缓存 1024 (THUMBNAILONLY)", 1024, SIIGBF_THUMBNAILONLY | SIIGBF_BIGGERSIZEOK),
            ("缓存 2048 (THUMBNAILONLY)", 2048, SIIGBF_THUMBNAILONLY | SIIGBF_BIGGERSIZEOK),
            ("允许现场解码 1024", 1024, SIIGBF_BIGGERSIZEOK),
        ];

        for (label, edge, flags) in cases {
            let started = Instant::now();
            match item.GetImage(SIZE { cx: edge, cy: edge }, flags) {
                Ok(hbmp) => {
                    let got = started.elapsed();
                    let (bw, bh) = bitmap_size(hbmp);
                    let started2 = Instant::now();
                    let data = hbitmap_to_bgra(hbmp, bw, bh);
                    let convert = started2.elapsed();
                    let valid = data.as_ref().is_some_and(|d| d.iter().any(|&b| b != 0));
                    let _ = DeleteObject(HGDIOBJ(hbmp.0));
                    println!(
                        "  {label:<26} {:>7.1}ms {bw}x{bh} 转换 {:>5.1}ms 有效={valid}",
                        got.as_secs_f64() * 1000.0,
                        convert.as_secs_f64() * 1000.0
                    );
                }
                Err(err) => println!("  {label:<26} 未命中 {:#x}", err.code().0),
            }
        }
    }
}

unsafe fn bitmap_size(hbmp: windows::Win32::Graphics::Gdi::HBITMAP) -> (i32, i32) {
    let mut bm = BITMAP::default();
    let n = unsafe {
        GetObjectW(
            HGDIOBJ(hbmp.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut _ as *mut _),
        )
    };
    if n > 0 {
        (bm.bmWidth, bm.bmHeight)
    } else {
        (0, 0)
    }
}

/// HBITMAP -> BGRA（gpui 的 RenderImage 正好吃 BGRA）
unsafe fn hbitmap_to_bgra(
    hbmp: windows::Win32::Graphics::Gdi::HBITMAP,
    w: i32,
    h: i32,
) -> Option<Vec<u8>> {
    if w <= 0 || h <= 0 {
        return None;
    }
    let mut bmi = BITMAPINFO::default();
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: w,
        biHeight: -h, // 负高度 = top-down
        biPlanes: 1,
        biBitCount: 32,
        biCompression: 0, // BI_RGB
        ..Default::default()
    };

    let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
    unsafe {
        let hdc = GetDC(None);
        let lines = GetDIBits(
            hdc,
            hbmp,
            0,
            h as u32,
            Some(buf.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, hdc);
        if lines > 0 { Some(buf) } else { None }
    }
}

/// image crate 基线：目前 egui 线走的路径
fn probe_image_crate(path: &str) {
    println!("\n-- image crate 全解码基线 (对照) --");
    let mut best = u128::MAX;
    for _ in 0..2 {
        let started = Instant::now();
        let decoded = (|| -> std::result::Result<image::DynamicImage, String> {
            let reader = image::ImageReader::open(path).map_err(|e| e.to_string())?;
            let reader = reader.with_guessed_format().map_err(|e| e.to_string())?;
            reader.decode().map_err(|e| e.to_string())
        })();
        match decoded {
            Ok(img) => {
                best = best.min(started.elapsed().as_micros());
                let dims = (img.width(), img.height());
                let rgba_started = Instant::now();
                let rgba = img.to_rgba8();
                println!(
                    "  {}x{} 解码 {:>7.1}ms, to_rgba8 {:>6.1}ms ({:.2}MB)",
                    dims.0,
                    dims.1,
                    best as f64 / 1000.0,
                    rgba_started.elapsed().as_secs_f64() * 1000.0,
                    rgba.len() as f64 / 1_048_576.0
                );
                return;
            }
            Err(err) => {
                println!("  解码失败: {err}");
                return;
            }
        }
    }
}
