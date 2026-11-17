//! 自绘菜单条 + 原生下拉菜单
//!
//! gpui 的窗口走 DirectComposition（`WS_EX_NOREDIRECTIONBITMAP`），非客户区没有
//! 重定向表面，`SetMenu` 挂上的 HMENU 菜单条**无法绘制**。所以菜单条由 gpui 自绘，
//! 下拉则交给 `TrackPopupMenuEx`——它是独立的 popup 窗口，不受该限制，可以拿到
//! 完整的系统菜单外观与键盘行为。

use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, PostMessageW, SetForegroundWindow, TrackPopupMenuEx,
    HMENU, MF_SEPARATOR, MF_STRING, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_TOPALIGN, WM_NULL,
};

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
    Fullscreen,
    // 查看 → 窗口背景
    BackdropOpaque,
    BackdropMica,
    BackdropMicaAlt,
    BackdropBlurred,
    BackdropTransparent,
    // 设置 / 帮助
    Settings,
    Shortcuts,
    About,
    CheckUpdate,
}

/// 一个顶层菜单
pub struct MenuSpec {
    pub title: &'static str,
    /// （命令 ID, 文案, 命令）；ID 为 0 表示分隔线；空表示顶层命令按钮
    pub items: &'static [(usize, &'static str, Cmd)],
}

pub const MENUS: &[MenuSpec] = &[
    MenuSpec {
        title: "文件",
        items: &[
            (1001, "打开文件(&O)...\tCtrl+O", Cmd::Open),
            (0, "", Cmd::Open),
            (1002, "退出(&X)\tAlt+F4", Cmd::Exit),
        ],
    },
    MenuSpec {
        title: "查看",
        items: &[
            (1010, "适应窗口(&0)", Cmd::Fit),
            (1011, "原始大小(&1)", Cmd::Original),
            (1012, "填充窗口(&2)", Cmd::Fill),
            (0, "", Cmd::Fit),
            (1013, "全屏(&U)\tF11", Cmd::Fullscreen),
            (0, "", Cmd::Fit),
            (1020, "不透明背景", Cmd::BackdropOpaque),
            (1021, "Mica 系统材质", Cmd::BackdropMica),
            (1022, "Mica Alt 材质", Cmd::BackdropMicaAlt),
            (1023, "亚克力模糊", Cmd::BackdropBlurred),
            (1024, "完全透明", Cmd::BackdropTransparent),
        ],
    },
    // 顶层命令按钮（无下拉）
    MenuSpec {
        title: "设置",
        items: &[],
    },
    MenuSpec {
        title: "帮助",
        items: &[
            (1040, "快捷键(&K)", Cmd::Shortcuts),
            (1041, "关于 FastView(&A)", Cmd::About),
            (0, "", Cmd::About),
            (1042, "检查更新(&U)", Cmd::CheckUpdate),
        ],
    },
];

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn append(menu: HMENU, id: usize, text: &str) {
    if id == 0 {
        unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null()) };
    } else {
        let text = wide(text);
        unsafe { AppendMenuW(menu, MF_STRING, id, text.as_ptr()) };
    }
}

/// 在客户区坐标 (x, y) 处弹出原生下拉菜单，返回选中的命令
///
/// 注意：`TrackPopupMenuEx` 会进入系统菜单模态循环，阻塞调用线程直到用户取消或选中。
pub fn popup_at_client(hwnd: HWND, x: f32, y: f32, items: &[(usize, &'static str, Cmd)]) -> Option<Cmd> {
    if items.is_empty() {
        return None;
    }

    unsafe {
        let mut origin = POINT {
            x: x.round() as i32,
            y: y.round() as i32,
        };
        if ClientToScreen(hwnd, &mut origin) == 0 {
            return None;
        }

        let menu = CreatePopupMenu();
        for (id, text, _) in items {
            append(menu, *id, text);
        }

        // 没有前台权限时，点击菜单外部不会自动收起
        SetForegroundWindow(hwnd);

        let chosen = TrackPopupMenuEx(
            menu,
            TPM_RETURNCMD | TPM_LEFTALIGN | TPM_TOPALIGN,
            origin.x,
            origin.y,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
        PostMessageW(hwnd, WM_NULL, 0, 0);

        items
            .iter()
            .find(|(id, _, _)| *id == chosen as usize)
            .map(|(_, _, cmd)| *cmd)
    }
}
