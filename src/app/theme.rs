//! 配色与界面尺寸
//!
//! 主题跟随系统（可被配置强制），标题栏与主体共用同一套半透明底色：
//! 背景层 alpha 由配置的「不透明度」决定，DWM 亚克力/云母在下面透出。
//! 关闭材质时 alpha 为 1，退化为实色，同时恢复 subpixel 文字。

use gpui::{Hsla, WindowAppearance, hsla, rgb};

use super::settings::Theme;

/// Win11 标准标题栏高度
pub const TITLEBAR_HEIGHT: f32 = 32.0;
/// Win11 标准窗口按钮宽度
pub const WINDOW_BUTTON_WIDTH: f32 = 46.0;
/// 窗口控制图标的占用尺寸（Win11 的几何图形都是 10×10）
pub const ICON_SIZE: f32 = 10.0;
/// 关闭按钮悬停 / 按下的红色（Win11 原生值）
pub const CLOSE_HOVER: u32 = 0xc42b1c;
pub const CLOSE_ACTIVE: u32 = 0xb0271a;
/// 下拉菜单面板宽度：容纳最长标签 + 右对齐的快捷键列
pub const MENU_WIDTH: f32 = 248.0;
/// 下拉菜单条目高度
pub const MENU_ITEM_HEIGHT: f32 = 32.0;
/// 缩略图条高度：显示时从视口高度里扣掉
pub const THUMB_BAR_HEIGHT: f32 = 88.0;
/// 单个缩略图边长
pub const THUMB_SIZE: f32 = 64.0;
/// 菜单 / 浮层圆角
pub const RADIUS_PANEL: f32 = 8.0;
/// 菜单条目 / 控件圆角
pub const RADIUS_ITEM: f32 = 4.0;
/// 系统强调色取不到时的回退（Win11 默认蓝）
pub const DEFAULT_ACCENT: u32 = 0x0078_d4;

/// 明暗判定：配置优先，`System` 跟随窗口外观
pub fn is_dark(theme: Theme, appearance: WindowAppearance) -> bool {
    match theme {
        Theme::Dark => true,
        Theme::Light => false,
        Theme::System => matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
    }
}

/// 跟随明暗与透明度的配色
#[derive(Clone, Copy)]
pub struct Palette {
    pub text: Hsla,
    pub text_muted: Hsla,
    /// 窗口控制图标（几何描边）
    pub icon: Hsla,
    /// 系统强调色：选中环、滑杆、主按钮
    pub accent: Hsla,
    /// 标题栏底色（0xRRGGBB），实色模式下同时用作 DWM 边框色
    pub titlebar_rgb: u32,
    /// 标题栏底色
    pub titlebar: Hsla,
    /// 内容区底色（图片周围）
    pub content_bg: Hsla,
    /// 缩略图条底色
    pub surface_bg: Hsla,
    /// 浮在图片上的卡片（状态条）
    pub card_bg: Hsla,
    pub card_border: Hsla,
    /// 下拉菜单 / 设置面板：比背景更实，保证可读
    pub menu_bg: Hsla,
    pub menu_border: Hsla,
    /// 分隔线
    pub separator: Hsla,
    pub hover: Hsla,
    pub active: Hsla,
    /// 滑杆 / 未填充轨道
    pub track: Hsla,
}

impl Palette {
    /// `alpha` 是背景层不透明度（0..=1），`accent` 是 `0xRRGGBB` 系统强调色
    pub fn new(dark: bool, alpha: f32, accent: u32) -> Self {
        let alpha = alpha.clamp(0.0, 1.0);
        let accent: Hsla = rgb(accent & 0x00ff_ffff).into();

        if dark {
            // 标题栏比主体略实一点，靠这点差异区分，不加分隔线
            let titlebar_alpha = (alpha + 0.05).min(1.0);
            Self {
                text: hsla(0.0, 0.0, 1.0, 0.92),
                text_muted: hsla(0.0, 0.0, 1.0, 0.60),
                icon: hsla(0.0, 0.0, 1.0, 0.90),
                accent,
                titlebar_rgb: 0x24_24_24,
                titlebar: rgb(0x24_24_24).opacity(titlebar_alpha).into(),
                content_bg: rgb(0x1c_1c1c & 0x00ff_ffff).opacity(alpha).into(),
                surface_bg: rgb(0x24_24_24).opacity(titlebar_alpha).into(),
                card_bg: rgb(0x2c_2c_2c).opacity(0.81).into(),
                card_border: hsla(0.0, 0.0, 1.0, 0.12),
                menu_bg: rgb(0x2c_2c_2c).opacity(alpha.max(0.88)).into(),
                menu_border: hsla(0.0, 0.0, 1.0, 0.10),
                separator: hsla(0.0, 0.0, 1.0, 0.09),
                hover: hsla(0.0, 0.0, 1.0, 0.06),
                active: hsla(0.0, 0.0, 1.0, 0.10),
                track: hsla(0.0, 0.0, 1.0, 0.18),
            }
        } else {
            // 浅色的标题栏比主体深一档且更实：半透明下只有拉开明度与 alpha，
            // 才不会因为壁纸透上来而和主体糊成一片（暗色同理但反差天然更大）
            let titlebar_alpha = (alpha + 0.20).min(1.0);
            Self {
                text: hsla(0.0, 0.0, 0.0, 0.90),
                text_muted: hsla(0.0, 0.0, 0.0, 0.55),
                icon: hsla(0.0, 0.0, 0.0, 0.86),
                accent,
                titlebar_rgb: 0xe0_e0_e0,
                titlebar: rgb(0xe0_e0_e0).opacity(titlebar_alpha).into(),
                content_bg: rgb(0xf6_f6_f6).opacity(alpha).into(),
                surface_bg: rgb(0xe0_e0_e0).opacity(titlebar_alpha).into(),
                card_bg: rgb(0xff_ff_ff).opacity(0.85).into(),
                card_border: hsla(0.0, 0.0, 0.0, 0.10),
                menu_bg: rgb(0xfa_fa_fa).opacity(alpha.max(0.90)).into(),
                menu_border: hsla(0.0, 0.0, 0.0, 0.08),
                separator: hsla(0.0, 0.0, 0.0, 0.07),
                hover: hsla(0.0, 0.0, 0.0, 0.04),
                active: hsla(0.0, 0.0, 0.0, 0.08),
                track: hsla(0.0, 0.0, 0.0, 0.14),
            }
        }
    }
}
