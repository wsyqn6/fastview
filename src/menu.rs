//! 菜单数据
//!
//! 下拉由 gpui 自绘（窗口内浮层），所以这里只放纯数据：命令、条目、快捷键、助记符。
//! `mnemonic` 只用于匹配按键，显示时由渲染层拼成 `文件(F)` 并给字母加下划线。

use std::ops::Range;

use crate::text::TextKey;

/// 菜单命令（对应 egui 版 src/ui/menu.rs 的菜单项）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    // 文件
    Open,
    Exit,
    // 查看
    Fit,
    Original,
    Fill,
    RotateCw,
    RotateCcw,
    Fullscreen,
    Thumbnails,
    PhotoInfo,
    // 设置 / 帮助
    Settings,
    Shortcuts,
    About,
    CheckUpdate,
}

/// 一个下拉条目
pub enum MenuItem {
    Separator,
    Command {
        label: TextKey,
        shortcut: &'static str,
        /// 菜单展开后直接按该键执行（原生行为：不需要 Alt）
        mnemonic: char,
        cmd: Cmd,
    },
}

use MenuItem::{Command, Separator};

/// 一个顶层菜单；`items` 为空表示顶层命令按钮（点击直接执行）
pub struct MenuSpec {
    pub title: TextKey,
    /// Alt+该键展开 / 切换，与 Windows 原生菜单一致
    pub mnemonic: char,
    pub items: &'static [MenuItem],
}

pub const MENUS: &[MenuSpec] = &[
    MenuSpec {
        title: TextKey::File,
        mnemonic: 'F',
        items: &[
            Command {
                label: TextKey::Open,
                shortcut: "Ctrl+O",
                mnemonic: 'O',
                cmd: Cmd::Open,
            },
            Separator,
            Command {
                label: TextKey::Exit,
                shortcut: "Alt+F4",
                mnemonic: 'X',
                cmd: Cmd::Exit,
            },
        ],
    },
    MenuSpec {
        title: TextKey::View,
        mnemonic: 'V',
        items: &[
            Command {
                label: TextKey::Fit,
                shortcut: "0",
                mnemonic: '0',
                cmd: Cmd::Fit,
            },
            Command {
                label: TextKey::OriginalSize,
                shortcut: "1",
                mnemonic: '1',
                cmd: Cmd::Original,
            },
            Command {
                label: TextKey::Fill,
                shortcut: "2",
                mnemonic: '2',
                cmd: Cmd::Fill,
            },
            Separator,
            Command {
                label: TextKey::RotateCw,
                shortcut: "R",
                mnemonic: 'R',
                cmd: Cmd::RotateCw,
            },
            Command {
                label: TextKey::RotateCcw,
                shortcut: "Shift+R",
                mnemonic: 'N',
                cmd: Cmd::RotateCcw,
            },
            Separator,
            Command {
                label: TextKey::Fullscreen,
                shortcut: "F / F11",
                mnemonic: 'U',
                cmd: Cmd::Fullscreen,
            },
            Command {
                label: TextKey::Thumbnails,
                shortcut: "T",
                mnemonic: 'T',
                cmd: Cmd::Thumbnails,
            },
            Command {
                label: TextKey::PhotoInfo,
                shortcut: "I",
                mnemonic: 'I',
                cmd: Cmd::PhotoInfo,
            },
        ],
    },
    // 顶层命令按钮（无下拉）
    MenuSpec {
        title: TextKey::Settings,
        mnemonic: 'S',
        items: &[],
    },
    MenuSpec {
        title: TextKey::Help,
        mnemonic: 'H',
        items: &[
            Command {
                label: TextKey::Shortcuts,
                shortcut: "",
                mnemonic: 'K',
                cmd: Cmd::Shortcuts,
            },
            Command {
                label: TextKey::About,
                shortcut: "",
                mnemonic: 'A',
                cmd: Cmd::About,
            },
            Separator,
            Command {
                label: TextKey::CheckUpdate,
                shortcut: "",
                mnemonic: 'U',
                cmd: Cmd::CheckUpdate,
            },
        ],
    },
];

/// Alt+键 对应的顶层菜单
pub fn menu_by_mnemonic(key: char) -> Option<usize> {
    MENUS
        .iter()
        .position(|spec| spec.mnemonic.eq_ignore_ascii_case(&key))
}

/// 菜单展开后，按键对应的条目命令
pub fn cmd_by_mnemonic(index: usize, key: char) -> Option<Cmd> {
    MENUS.get(index)?.items.iter().find_map(|item| match item {
        Command { mnemonic, cmd, .. } if mnemonic.eq_ignore_ascii_case(&key) => Some(*cmd),
        _ => None,
    })
}

/// 显示文案与助记符的字节区间：`文件(F)` → 倒数第二个字符 `F`
///
/// `mnemonic` 限定 ASCII，因此区间固定落在末尾两字节的前一字节上。
pub fn mnemonic_label(label: &str, mnemonic: char) -> (String, Range<usize>) {
    let text = format!("{label}({})", mnemonic.to_ascii_uppercase());
    let start = text.len() - 2;
    (text, start..start + 1)
}
