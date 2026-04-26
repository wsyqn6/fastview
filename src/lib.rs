//! FastView - 快速图片查看器
//!
//! 一个专注于图片查看的轻量级应用，启动速度 <1s，体积 ~5MB。

// 仅在 debug 模式下输出日志
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        eprintln!($($arg)*);
    };
}

// 性能监控宏 (仅 debug 模式)
#[macro_export]
macro_rules! perf_log {
    ($label:expr, $start:expr) => {
        #[cfg(debug_assertions)]
        eprintln!("[PERF] {}: {}ms", $label, $start.elapsed().as_millis());
    };
}

pub mod app;

// 核心业务逻辑模块
pub mod core {
    pub mod i18n;
    pub mod loader;
    pub mod thumbnail;
    pub mod thumbnail_cache;
    pub mod types;

    // 重新导出常用类型
    pub use i18n::TextKey;
    pub use loader::{LoadCommand, LoadPriority, LoadResult};
    pub use types::*;
}

// 事件处理器模块
pub mod handler {
    pub mod events;
    pub mod keyboard;
}

// 业务操作模块
pub mod operation {
    pub mod cache_manager;
    pub mod image_ops;
    pub mod navigation;
    pub mod tile_renderer;
}

// UI 渲染模块
pub mod ui {
    pub mod dialogs;
    pub mod fonts;
    pub mod image;
    pub mod lifecycle;
    pub mod menu;
    pub mod status;
    pub mod thumbnail_manager;

    // 重新导出常用函数
    pub use menu::render_menu_bar;
    pub use status::render_status_content;
}

// 工具宏
pub mod utils;
