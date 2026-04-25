/// UI 渲染模块
pub mod fonts;
pub mod menu;
pub mod status;
pub mod image;
pub mod dialogs;
pub mod lifecycle;

// 重新导出常用函数
pub use menu::render_menu_bar;
pub use status::render_status_content;
