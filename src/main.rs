//! FastView gpui 骨架（spike）
//!
//! 已落地的 Windows 体验：
//! 1. 现代窗口 UI：自绘标题栏，菜单与窗口按钮都在标题栏内，不额外占一行（同 VS Code），
//!    并保留 DWM 原生动画、拖动分屏、贴靠布局；
//! 2. 菜单：入口与下拉都在窗口内自绘，和标题栏共用同一套配色与字号；
//! 3. 窗口外观：跟随系统的深浅色 + 亚克力 / 云母材质，不透明度可调（默认 65%），
//!    配置见 `%APPDATA%\fastview\gpui-settings.json`，`Ctrl+,` 打开设置面板；
//! 4. 图片查看：按视口选档的 WIC 缩放解码 → `RenderImage` → 适应/原始/填充、滚轮缩放、拖拽平移。
//!
//! 应用实现见 `app` 模块；首帧不做整图解码（见 `decode` 模块）。

// debug 保留终端看日志；release 隐藏控制台，日志默认写文件
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod archive;
#[cfg(windows)]
mod assoc;
mod cache;
#[cfg(windows)]
mod com;
mod decode;
mod folder;
mod log;
mod menu;
mod metadata;
#[cfg(windows)]
mod shell;
#[cfg(test)]
mod test_util;
mod text;
mod viewer;
#[cfg(windows)]
mod win;

fn main() {
    app::run();
}
