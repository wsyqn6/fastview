//! 配色与界面尺寸
//!
//! 标题栏与主体都是不透明实色，只靠明度差区分（同 VS Code / Windows Terminal），
//! 不加分隔线、不透出桌面：图片取色环境必须稳定。

use gpui::{Hsla, WindowAppearance, hsla, rgb};

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
pub const MENU_WIDTH: f32 = 232.0;
/// 缩略图条高度：显示时从视口高度里扣掉
pub const THUMB_BAR_HEIGHT: f32 = 88.0;
/// 单个缩略图边长
pub const THUMB_SIZE: f32 = 64.0;

/// 跟随系统深浅色的配色
#[derive(Clone, Copy)]
pub struct Palette {
    pub text: Hsla,
    pub text_muted: Hsla,
    /// 窗口控制图标（几何描边）
    pub icon: Hsla,
    /// 标题栏底色（0xRRGGBB），同时用作 DWM 边框色
    pub titlebar_rgb: u32,
    /// 内容区底色：不透明
    pub content_bg: Hsla,
    /// 浮在图片上的卡片（状态条）
    pub card_bg: Hsla,
    pub card_border: Hsla,
    /// 下拉菜单面板：不透明，避免透出图片干扰阅读
    pub menu_bg: Hsla,
    pub menu_border: Hsla,
    pub hover: Hsla,
    pub active: Hsla,
}

impl Palette {
    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        let light = matches!(
            appearance,
            WindowAppearance::Light | WindowAppearance::VibrantLight
        );

        if light {
            // 浅色：标题栏比内容区略深（VS Code Light 的做法）
            Self {
                text: hsla(0.0, 0.0, 0.10, 1.0),
                text_muted: hsla(0.0, 0.0, 0.0, 0.55),
                icon: hsla(0.0, 0.0, 0.18, 1.0),
                titlebar_rgb: 0xe6_e6_e6,
                content_bg: rgb(0xf7_f7_f7).into(),
                card_bg: rgb(0xff_ff_ff).opacity(0.88).into(),
                card_border: hsla(0.0, 0.0, 0.0, 0.14),
                menu_bg: rgb(0xff_ff_ff).into(),
                menu_border: hsla(0.0, 0.0, 0.0, 0.12),
                hover: hsla(0.0, 0.0, 0.0, 0.06),
                active: hsla(0.0, 0.0, 0.0, 0.10),
            }
        } else {
            // 深色：标题栏比内容区略亮
            Self {
                text: hsla(0.0, 0.0, 1.0, 0.94),
                text_muted: hsla(0.0, 0.0, 1.0, 0.60),
                icon: hsla(0.0, 0.0, 1.0, 0.90),
                titlebar_rgb: 0x2c_2c_2c,
                content_bg: rgb(0x18_18_18).into(),
                card_bg: rgb(0x2e_2e_2e).opacity(0.90).into(),
                card_border: hsla(0.0, 0.0, 1.0, 0.14),
                menu_bg: rgb(0x2b_2b_2b).into(),
                menu_border: hsla(0.0, 0.0, 1.0, 0.10),
                hover: hsla(0.0, 0.0, 1.0, 0.08),
                active: hsla(0.0, 0.0, 1.0, 0.12),
            }
        }
    }

    /// 标题栏底色
    pub fn titlebar(&self) -> Hsla {
        rgb(self.titlebar_rgb).into()
    }
}
