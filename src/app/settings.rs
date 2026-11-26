//! 用户配置：主题、窗口材质与透明度
//!
//! spike 使用独立文件：egui 主线也写 `%APPDATA%\fastview\settings.json`，
//! 共用一份会被对方保存时丢掉未知字段。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// 界面语言：默认跟随系统 UI 语言
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    System,
    Chinese,
    English,
}

impl Language {
    /// `System` 解析成具体语言；其余原样返回
    pub fn effective(self) -> Self {
        if self != Self::System {
            return self;
        }
        #[cfg(windows)]
        {
            use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;
            // LANGID 低 10 位为primary language id：0x09 英语，0x04 中文
            let primary = unsafe { GetUserDefaultUILanguage() } & 0x03ff;
            if primary == 0x09 {
                Self::English
            } else {
                Self::Chinese
            }
        }
        #[cfg(not(windows))]
        Self::Chinese
    }
}

/// 主题：默认跟随系统
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// 窗口背景材质：只决定是否请求系统 backdrop，不影响透明度
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Material {
    /// DWM 亚克力（系统瞬态背景），默认
    #[default]
    Acrylic,
    /// DWM 云母，开销更低
    Mica,
    /// 不请求系统 backdrop；低透明度时只保留普通透明，不模糊
    None,
}

/// 自绘背景层不透明度（百分比）
fn default_opacity() -> u8 {
    65
}

/// 打开图片时窗口跟随图片尺寸
fn default_auto_window_size() -> bool {
    true
}

/// 配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub language: Language,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub material: Material,
    #[serde(default = "default_opacity")]
    pub opacity: u8,
    #[serde(default)]
    pub show_thumbs: bool,
    #[serde(default)]
    pub status_pinned: bool,
    #[serde(default)]
    pub hide_chrome: bool,
    #[serde(default = "default_auto_window_size")]
    pub auto_window_size: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: Language::System,
            theme: Theme::System,
            material: Material::Acrylic,
            opacity: default_opacity(),
            show_thumbs: false,
            status_pinned: false,
            hide_chrome: false,
            auto_window_size: default_auto_window_size(),
        }
    }
}

impl Settings {
    /// 读取配置：文件不存在或解析失败时回退默认值
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => return Self::default(),
        };
        match serde_json::from_str::<Self>(&text) {
            Ok(mut settings) => {
                settings.clamp_opacity();
                settings
            }
            Err(error) => {
                crate::spike_log!("[SPIKE] settings parse failed: {error}");
                Self::default()
            }
        }
    }

    /// 写盘：失败只打日志，不影响运行
    pub fn save(&self) {
        let Some(path) = Self::path() else {
            return;
        };
        if let Some(parent) = path.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            crate::spike_log!("[SPIKE] settings dir failed: {error}");
            return;
        }
        match serde_json::to_string_pretty(self) {
            Ok(text) => {
                if let Err(error) = std::fs::write(&path, text) {
                    crate::spike_log!("[SPIKE] settings write failed: {error}");
                }
            }
            Err(error) => crate::spike_log!("[SPIKE] settings encode failed: {error}"),
        }
    }

    /// 自绘背景层 alpha（0..=1），只由不透明度决定
    pub fn alpha(&self) -> f32 {
        f32::from(self.opacity.min(100)) / 100.0
    }

    /// 是否请求系统背景模糊（与透明度无关）
    pub fn uses_backdrop(&self) -> bool {
        self.material != Material::None
    }

    /// 自绘背景层是否透明；100% 时窗口用 Opaque，恢复 subpixel AA
    pub fn translucent(&self) -> bool {
        self.opacity < 100
    }

    /// 配置里手写了 >100 的不透明度时，加载阶段收回到合法范围
    fn clamp_opacity(&mut self) {
        self.opacity = self.opacity.min(100);
    }

    fn path() -> Option<PathBuf> {
        Some(
            dirs::config_dir()?
                .join("fastview")
                .join("gpui-settings.json"),
        )
    }
}
