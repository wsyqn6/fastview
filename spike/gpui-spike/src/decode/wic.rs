//! WIC 后端：只负责「按目标尺寸快速解码」
//!
//! JPEG 走 DCT 缩放（1/2、1/4、1/8），解码成本随目标分辨率下降，这是「渐进式查看」的基础。
//! 方向、色彩管理都不在这里处理，统一由 `mod.rs` 负责。

use std::cell::Cell;
use std::path::Path;

use windows::core::{HSTRING, Interface};
use windows::Win32::Foundation::GENERIC_READ;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory2, GUID_WICPixelFormat32bppBGRA, IWICBitmapSourceTransform,
    IWICImagingFactory, WICBitmapTransformRotate0, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};

thread_local! {
    /// 每个线程只初始化一次 COM (MTA)；失败不重试，交给调用方兜底
    static COM_READY: Cell<bool> = const { Cell::new(false) };
}

fn ensure_com() -> Result<(), String> {
    COM_READY.with(|ready| {
        if ready.get() {
            return Ok(());
        }
        // MTA 重复初始化返回 S_FALSE，无副作用
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(|err| format!("COM 初始化失败：{err}"))?;
        ready.set(true);
        Ok(())
    })
}

fn err(error: windows::core::Error) -> String {
    format!("WIC: {error}")
}

/// 解码到接近 `(tw, th)` 的尺寸，返回 BGRA 像素（自上而下）
///
/// 若格式不支持缩放解码（PNG 等），WIC 仍会正常工作，只是退化为全解码 + 缩放。
pub fn decode_scaled(path: &Path, tw: u32, th: u32) -> Result<(Vec<u8>, u32, u32), String> {
    ensure_com()?;

    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory2, None, CLSCTX_INPROC_SERVER).map_err(err)?;
        let wide = HSTRING::from(path.to_string_lossy().as_ref());
        let decoder = factory
            .CreateDecoderFromFilename(&wide, None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)
            .map_err(err)?;
        let frame = decoder.GetFrame(0).map_err(err)?;

        // JPEG 的 frame 实现 IWICBitmapSourceTransform，才能用 DCT 缩放；
        // 不支持的格式必须交给 `image` 兜底 —— 这里返回全尺寸会撑爆 gpui 的纹理上限
        let transform = frame
            .cast::<IWICBitmapSourceTransform>()
            .map_err(|_| "WIC: 该格式不支持缩放解码".to_string())?;

        let mut width = tw.max(1);
        let mut height = th.max(1);
        let _ = transform.GetClosestSize(&mut width, &mut height);
        let stride = width * 4;
        let mut buf = vec![0u8; (stride * height) as usize];
        transform
            .CopyPixels(
                std::ptr::null(),
                width,
                height,
                &GUID_WICPixelFormat32bppBGRA,
                WICBitmapTransformRotate0,
                stride,
                &mut buf,
            )
            .map_err(err)?;
        Ok((buf, width, height))
    }
}
