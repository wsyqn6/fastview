//! 配色与界面尺寸
//!
//! 主题跟随系统（可被配置强制），标题栏与主体共用同一套半透明底色：
//! 背景层 alpha 由配置的「不透明度」决定，DWM 亚克力/云母在下面透出。
//! 关闭材质时 alpha 为 1，退化为实色，同时恢复 subpixel 文字。
//! 所有可见颜色都从这里取，绘制代码不写死色值。

use gpui::{Hsla, WindowAppearance, hsla, rgb};

use super::settings::Theme;

/// Win11 标准标题栏高度
pub const TITLEBAR_HEIGHT: f32 = 32.0;
/// Win11 标准窗口按钮宽度
pub const WINDOW_BUTTON_WIDTH: f32 = 46.0;
/// 窗口控制图标的占用尺寸（Win11 的几何图形都是 10×10）
pub const ICON_SIZE: f32 = 10.0;
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
    /// 当前是否为暗色，替代各处用 `text.l` 反推
    pub dark: bool,
    pub text: Hsla,
    pub text_muted: Hsla,
    /// 窗口控制图标（几何描边）
    pub icon: Hsla,
    /// 系统强调色：选中环、滑杆、主按钮
    pub accent: Hsla,
    /// 强调色底上的文字（按钮、开关圆点）
    pub on_accent: Hsla,
    /// 错误提示文字
    pub error: Hsla,
    /// 关闭按钮悬停 / 按下（Win11 原生红）
    pub close_hover: Hsla,
    pub close_active: Hsla,
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
    /// 浮层投影（外圈 / 贴边）
    pub shadow_strong: Hsla,
    pub shadow_soft: Hsla,
    /// 拖放提示：内发光 / 压暗层
    pub drop_glow: Hsla,
    pub drop_shade: Hsla,
    /// minimap 可视区描边：在任意照片上都要看得清
    pub minimap_mark: Hsla,
}

impl Palette {
    /// B 冷灰蓝；`alpha` 是背景层不透明度（0..=1），`accent` 是 `0xRRGGBB` 系统强调色
    pub fn new(dark: bool, alpha: f32, accent: u32) -> Self {
        let alpha = alpha.clamp(0.0, 1.0);
        let accent: Hsla = rgb(accent & 0x00ff_ffff).into();

        if dark {
            // 标题栏比主体略实一点，靠这点差异区分，不加分隔线
            let titlebar_alpha = (alpha + 0.05).min(1.0);
            Self {
                dark: true,
                text: rgb(0xF2_F4_F7).opacity(0.92).into(),
                text_muted: rgb(0xB6_BC_C6).opacity(0.68).into(),
                icon: rgb(0xF2_F4_F7).opacity(0.90).into(),
                accent,
                on_accent: hsla(0.0, 0.0, 1.0, 1.0),
                error: hsla(0.0, 0.72, 0.60, 1.0),
                close_hover: rgb(0xc4_2b_1c).into(),
                close_active: rgb(0xb0_27_1a).into(),
                titlebar_rgb: 0x29_2B_2F,
                titlebar: rgb(0x29_2B_2F).opacity(titlebar_alpha).into(),
                content_bg: rgb(0x1E_20_23).opacity(alpha).into(),
                surface_bg: rgb(0x29_2B_2F).opacity(titlebar_alpha).into(),
                card_bg: rgb(0x34_37_3B).opacity(0.88).into(),
                card_border: rgb(0xFF_FF_FF).opacity(0.12).into(),
                menu_bg: rgb(0x34_37_3B).opacity(alpha.max(0.90)).into(),
                menu_border: rgb(0xFF_FF_FF).opacity(0.10).into(),
                separator: rgb(0xFF_FF_FF).opacity(0.08).into(),
                hover: rgb(0xFF_FF_FF).opacity(0.06).into(),
                active: rgb(0xFF_FF_FF).opacity(0.10).into(),
                track: rgb(0xFF_FF_FF).opacity(0.18).into(),
                shadow_strong: hsla(0.0, 0.0, 0.0, 0.24),
                shadow_soft: hsla(0.0, 0.0, 0.0, 0.14),
                drop_glow: rgb(0xFF_FF_FF).opacity(0.18).into(),
                drop_shade: hsla(0.0, 0.0, 0.0, 0.05),
                minimap_mark: rgb(0xFF_FF_FF).opacity(0.85).into(),
            }
        } else {
            // 浅色的标题栏比主体深一档且更实：半透明下只有拉开明度与 alpha，
            // 才不会因为壁纸透上来而和主体糊成一片（暗色同理但反差天然更大）
            let titlebar_alpha = (alpha + 0.20).min(1.0);
            Self {
                dark: false,
                text: rgb(0x1B_1F_24).opacity(0.90).into(),
                text_muted: rgb(0x62_6A_75).opacity(0.64).into(),
                icon: rgb(0x1B_1F_24).opacity(0.86).into(),
                accent,
                on_accent: hsla(0.0, 0.0, 1.0, 1.0),
                error: hsla(0.0, 0.72, 0.60, 1.0),
                close_hover: rgb(0xc4_2b_1c).into(),
                close_active: rgb(0xb0_27_1a).into(),
                titlebar_rgb: 0xEC_EE_F1,
                titlebar: rgb(0xEC_EE_F1).opacity(titlebar_alpha).into(),
                content_bg: rgb(0xF7_F8_FA).opacity(alpha).into(),
                surface_bg: rgb(0xEC_EE_F1).opacity(titlebar_alpha).into(),
                card_bg: rgb(0xFF_FF_FF).opacity(0.90).into(),
                card_border: hsla(0.0, 0.0, 0.0, 0.10),
                menu_bg: rgb(0xFF_FF_FF).opacity(alpha.max(0.92)).into(),
                menu_border: hsla(0.0, 0.0, 0.0, 0.08),
                separator: hsla(0.0, 0.0, 0.0, 0.07),
                hover: hsla(0.0, 0.0, 0.0, 0.04),
                active: hsla(0.0, 0.0, 0.0, 0.08),
                track: hsla(0.0, 0.0, 0.0, 0.14),
                shadow_strong: hsla(0.0, 0.0, 0.0, 0.24),
                shadow_soft: hsla(0.0, 0.0, 0.0, 0.14),
                drop_glow: hsla(0.0, 0.0, 0.0, 0.12),
                drop_shade: hsla(0.0, 0.0, 0.0, 0.05),
                minimap_mark: hsla(0.0, 0.0, 0.0, 0.85),
            }
        }
    }

}
