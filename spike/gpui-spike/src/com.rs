//! 线程内 COM 初始化
//!
//! WIC 与 Shell 接口都在后台解码线程上使用，统一用 MTA 初始化：
//! 同一线程不能既 STA 又 MTA，分散初始化会撞 `RPC_E_CHANGED_MODE`。

use std::cell::Cell;

use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};

thread_local! {
    static READY: Cell<bool> = const { Cell::new(false) };
}

/// 当前线程是否已初始化 COM（MTA）
pub fn ensure() -> Result<(), String> {
    READY.with(|ready| {
        if ready.get() {
            return Ok(());
        }
        // 已是 MTA 时返回 S_FALSE，同样视为成功
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(|err| format!("COM 初始化失败：{err}"))?;
        ready.set(true);
        Ok(())
    })
}
