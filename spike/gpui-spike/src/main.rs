//! FastView gpui 骨架（spike）
//!
//! 已落地的 Windows 体验：
//! 1. 现代窗口 UI：自绘标题栏，菜单与窗口按钮都在标题栏内，不额外占一行（同 VS Code），
//!    并保留 DWM 原生动画、拖动分屏、贴靠布局；
//! 2. 菜单：入口与下拉都在窗口内自绘，和标题栏共用同一套配色与字号；
//! 3. 图片查看：按视口选档的 WIC 缩放解码 → `RenderImage` → 适应/原始/填充、滚轮缩放、拖拽平移。
//!
//! 分块渲染与缓存尚未接入；首帧不做整图解码（见 `decode` 模块）。

mod decode;
#[cfg(windows)]
mod menu;
mod viewer;
#[cfg(windows)]
mod win;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use gpui::{
    AnyElement, App, Bounds, BoxShadow, Context, Div, FocusHandle, HighlightStyle, Hsla,
    KeyDownEvent, ModifiersChangedEvent, MouseButton, MouseDownEvent, MouseMoveEvent, ObjectFit,
    Path, PathPromptOptions, Pixels, Point, ScrollDelta, ScrollEvent, Size, StyledText,
    Subscription, TitlebarOptions, UnderlineStyle, Window, WindowAppearance,
    WindowBackgroundAppearance, WindowBounds, WindowControlArea, WindowOptions, canvas, div, hsla,
    img, point, prelude::*, px, rgb, size,
};
use gpui_platform::application;

const TITLEBAR_HEIGHT: f32 = 32.0;
/// Win11 标准窗口按钮宽度
const WINDOW_BUTTON_WIDTH: f32 = 46.0;
/// 窗口控制图标的占用尺寸（Win11 的几何图形都是 10×10）
const ICON_SIZE: f32 = 10.0;
/// 关闭按钮的红色（Win11 原生值）
const CLOSE_HOVER: u32 = 0xc42b1c;
const CLOSE_ACTIVE: u32 = 0xb0271a;
/// 下拉菜单面板宽度：容纳最长标签 + 右对齐的快捷键列
const MENU_WIDTH: f32 = 232.0;

/// 跟随系统深浅色的配色
///
/// 标题栏与主体都是不透明实色，只靠明度差区分（同 VS Code / Windows Terminal），
/// 不加分隔线、不透出桌面：图片取色环境必须稳定。
#[derive(Clone, Copy)]
struct Palette {
    text: Hsla,
    text_muted: Hsla,
    /// 窗口控制图标（几何描边）
    icon: Hsla,
    /// 标题栏底色（0xRRGGBB），同时用作 DWM 边框色
    titlebar_rgb: u32,
    /// 内容区底色：不透明
    content_bg: Hsla,
    /// 浮在图片上的卡片（状态条）
    card_bg: Hsla,
    card_border: Hsla,
    /// 下拉菜单面板：不透明，避免透出图片干扰阅读
    menu_bg: Hsla,
    menu_border: Hsla,
    hover: Hsla,
    active: Hsla,
}

impl Palette {
    fn for_appearance(appearance: WindowAppearance) -> Self {
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
    fn titlebar(&self) -> Hsla {
        rgb(self.titlebar_rgb).into()
    }
}

struct Spike {
    /// 原生 HWND（0 表示不可用），用于设置 DWM 边框 / 圆角
    hwnd: isize,
    started_at: Instant,
    first_frame_logged: bool,
    /// 系统深浅色变化时通知重绘（首帧后注册）
    _appearance_sub: Option<Subscription>,
    /// 窗口真正显示时 DWM 会重建框架，需要再压一次边框设置
    _visibility_sub: Option<Subscription>,
    /// 当前图片与查看状态
    image: Option<viewer::LoadedImage>,
    view: viewer::ViewState,
    loading: bool,
    error: Option<String>,
    /// 加载序号：后发起的请求覆盖先前的，避免旧结果回写
    load_id: u64,
    /// 拖拽平移的上一帧位置
    drag_last: Option<Point<gpui::Pixels>>,
    /// 键盘快捷键
    focus_handle: FocusHandle,
    /// 上一次的窗口状态（最大化、全屏）：变化后 DWM 会重算框架，需要重设边框
    window_state: (bool, bool),
    /// 当前展开的顶层菜单（索引对应 `menu::MENUS`）
    open_menu: Option<usize>,
    /// 下拉里键盘选中的条目（`spec.items` 的索引）
    menu_selected: Option<usize>,
    /// 顶层菜单项在窗口坐标里的位置（prepaint 每帧更新，用于定位下拉与切换）
    menu_rects: Rc<RefCell<Vec<Bounds<Pixels>>>>,
    /// Alt 是否按住：按住才显示助记符下划线（同系统菜单）
    alt_held: bool,
    /// 窗口失焦时收起菜单（原生菜单同行为）
    _activation_sub: Option<Subscription>,
}

/// 窗口控制图标类型
#[derive(Clone, Copy)]
enum Icon {
    Minimize,
    Maximize,
    Restore,
    Close,
}

/// 把一条线段展开成有宽度的矩形，作为两个三角形推进 `Path`
///
/// gpui 的 `Path` 只有填充、没有描边，1px 线条得自己算四个顶点。
fn push_stroke(path: &mut Path<Pixels>, from: Point<Pixels>, to: Point<Pixels>, width: f32) {
    let fx: f32 = from.x.into();
    let fy: f32 = from.y.into();
    let tx: f32 = to.x.into();
    let ty: f32 = to.y.into();
    let dx = tx - fx;
    let dy = ty - fy;
    let len = (dx * dx + dy * dy).sqrt();
    if len < f32::EPSILON {
        return;
    }

    let half = width / 2.0;
    let nx = -dy / len * half;
    let ny = dx / len * half;

    let a = point(px(fx + nx), px(fy + ny));
    let b = point(px(tx + nx), px(ty + ny));
    let c = point(px(tx - nx), px(ty - ny));
    let d = point(px(fx - nx), px(fy - ny));
    let st = (point(0.0f32, 0.0), point(1.0, 0.0), point(1.0, 1.0));
    path.push_triangle((a, b, c), st);
    path.push_triangle((a, c, d), st);
}

/// 描边矩形（四条边分别展开）
fn push_stroke_rect(path: &mut Path<Pixels>, left: f32, top: f32, right: f32, bottom: f32) {
    let width = 1.0;
    push_stroke(path, point(px(left), px(top)), point(px(right), px(top)), width);
    push_stroke(path, point(px(right), px(top)), point(px(right), px(bottom)), width);
    push_stroke(path, point(px(right), px(bottom)), point(px(left), px(bottom)), width);
    push_stroke(path, point(px(left), px(bottom)), point(px(left), px(top)), width);
}

/// 只取单字符键，用来区分助记符与 `f11` / `space` 这类多字符键名
fn single_char(key: &str) -> Option<char> {
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}

/// 助记符文本：`show_underline` 时字母加下划线，其余样式继承父级
///
/// 下划线颜色必须显式给：`UnderlineStyle.color` 为 `None` 时回退到透明的默认色。
#[cfg(windows)]
fn mnemonic_label(label: &str, mnemonic: char, color: Hsla, show_underline: bool) -> AnyElement {
    let (text, range) = menu::mnemonic_label(label, mnemonic);
    let styled = StyledText::new(text);
    if !show_underline {
        return styled.into_any_element();
    }

    let underline = HighlightStyle {
        underline: Some(UnderlineStyle {
            thickness: px(1.0),
            color: Some(color),
            wavy: false,
        }),
        ..Default::default()
    };

    styled
        .with_highlights([(range, underline)])
        .into_any_element()
}

/// 在给定中心绘制图标
///
/// 线条中心对到整数逻辑像素，1px 描边才不会发虚。
fn paint_icon(window: &mut Window, center: Point<Pixels>, icon: Icon, color: Hsla) {
    let cx: f32 = f32::from(center.x).round();
    let cy: f32 = f32::from(center.y).round();
    let mut path = Path::new(point(px(cx), px(cy)));

    match icon {
        Icon::Minimize => push_stroke(
            &mut path,
            point(px(cx - 5.0), px(cy)),
            point(px(cx + 5.0), px(cy)),
            1.0,
        ),
        // 描边中心在 4.5 时，外沿正好落在 ±5，整体 10×10
        Icon::Maximize => push_stroke_rect(&mut path, cx - 4.5, cy - 4.5, cx + 4.5, cy + 4.5),
        Icon::Restore => {
            // 前面那个窗口：左下方完整框
            push_stroke_rect(&mut path, cx - 4.5, cy - 2.0, cx + 2.5, cy + 5.0);
            // 后面那个窗口：只露上边与右边
            push_stroke(
                &mut path,
                point(px(cx - 2.0), px(cy - 4.5)),
                point(px(cx + 4.5), px(cy - 4.5)),
                1.0,
            );
            push_stroke(
                &mut path,
                point(px(cx + 4.5), px(cy - 4.5)),
                point(px(cx + 4.5), px(cy + 2.0)),
                1.0,
            );
        }
        Icon::Close => {
            push_stroke(
                &mut path,
                point(px(cx - 4.5), px(cy - 4.5)),
                point(px(cx + 4.5), px(cy + 4.5)),
                1.0,
            );
            push_stroke(
                &mut path,
                point(px(cx - 4.5), px(cy + 4.5)),
                point(px(cx + 4.5), px(cy - 4.5)),
                1.0,
            );
        }
    }

    window.paint_path(path, color);
}

/// 图标元素：统一尺寸与线宽，避免三个按钮看起来不是一套
fn icon_element(icon: Icon, color: Hsla) -> Div {
    div().w(px(ICON_SIZE)).h(px(ICON_SIZE)).child(
        canvas(
            |bounds, _, _| bounds.center(),
            move |_, center, window, _| paint_icon(window, center, icon, color),
        )
        .size_full(),
    )
}

/// Win11 风格的窗口控制按钮
///
/// 交给系统处理：`window_control_area` 会让 `WM_NCHITTEST` 返回 HTMINBUTTON /
/// HTMAXBUTTON / HTCLOSE，由 `DefWindowProc` 执行最小化、最大化（含 Snap Layouts）与关闭，
/// 因此这里不挂 on_click，避免和系统行为重复。
fn window_button(
    id: &'static str,
    icon: Icon,
    danger: bool,
    area: WindowControlArea,
    palette: Palette,
) -> AnyElement {
    let mut content = div()
        .relative()
        .w(px(ICON_SIZE))
        .h(px(ICON_SIZE))
        .child(icon_element(icon, palette.icon));

    if danger {
        // 悬停变红时图标转白：叠一层同形图标，靠 group_hover 淡入
        content = content.child(
            div()
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .opacity(0.0)
                .group_hover(id, |style| style.opacity(1.0))
                .child(icon_element(icon, hsla(0.0, 0.0, 1.0, 1.0))),
        );
    }

    div()
        .id(id)
        .group(id)
        .w(px(WINDOW_BUTTON_WIDTH))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .window_control_area(area)
        .hover(move |style| {
            if danger {
                style.bg(rgb(CLOSE_HOVER))
            } else {
                style.bg(palette.hover)
            }
        })
        .active(move |style| {
            if danger {
                style.bg(rgb(CLOSE_ACTIVE))
            } else {
                style.bg(palette.active)
            }
        })
        .child(content)
        .into_any_element()
}

impl Spike {
    /// 标题栏：左边菜单、右边窗口按钮，空白处可拖拽 / 双击最大化
    fn render_titlebar(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let is_maximized = window.is_maximized();
        let palette = Palette::for_appearance(window.appearance());
        // 助记符下划线：按住 Alt 或菜单展开时显示
        let mnemonics = self.alt_held || self.open_menu.is_some();

        let mut bar = div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(TITLEBAR_HEIGHT))
            // 标题栏与主体都是实色，靠明度差区分，不加分隔线
            .bg(palette.titlebar())
            // 左边缘留白也可拖动（需要显式 id 才会生成命中盒）
            .child(
                div()
                    .id("titlebar-drag-left")
                    .w(px(8.0))
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            );

        // 菜单项单独包一层：`on_children_prepainted` 的索引与 `MENUS` 一一对应，
        // 不受标题栏其它子元素（留白、窗口按钮）的数量与顺序影响
        #[cfg(windows)]
        {
            let mut menus = div().flex().flex_row().items_center();
            for (index, spec) in menu::MENUS.iter().enumerate() {
                let open = self.open_menu == Some(index);
                menus = menus.child(
                    div()
                        .id(("top-menu", index))
                        .h(px(24.0))
                        .px(px(10.0))
                        .flex()
                        .items_center()
                        .rounded(px(4.0))
                        .text_size(px(12.0))
                        .text_color(palette.text)
                        .when(open, |this| this.bg(palette.active))
                        .when(!open, |this| {
                            this.hover(move |style| style.bg(palette.hover))
                                .active(move |style| style.bg(palette.active))
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.press_menu(index, window, cx);
                            }),
                        )
                        .child(mnemonic_label(spec.title, spec.mnemonic, palette.text, mnemonics)),
                );
            }
            bar = bar.child(menus.on_children_prepainted({
                let rects = self.menu_rects.clone();
                move |bounds, _, _| *rects.borrow_mut() = bounds
            }));
        }

        // 空白区即拖拽区：拖动分屏、双击最大化都交给系统（HTCAPTION）
        bar.child(
            div()
                .id("titlebar-drag")
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .window_control_area(WindowControlArea::Drag)
                .child(
                    div()
                        .px(px(16.0))
                        .text_size(px(12.0))
                        .text_color(palette.text_muted)
                        .child(
                            self.image
                                .as_ref()
                                .map(|image| image.file_name())
                                .unwrap_or_default(),
                        ),
                ),
        )
        .child(window_button(
            "win-min",
            Icon::Minimize,
            false,
            WindowControlArea::Min,
            palette,
        ))
        .child(window_button(
            "win-max",
            if is_maximized {
                Icon::Restore
            } else {
                Icon::Maximize
            },
            false,
            WindowControlArea::Max,
            palette,
        ))
        .child(window_button(
            "win-close",
            Icon::Close,
            true,
            WindowControlArea::Close,
            palette,
        ))
        .into_any_element()
    }

    /// Alt+助记符：切换顶层菜单；未命中则收起已展开的菜单（同原生行为）
    #[cfg(windows)]
    fn press_mnemonic(&mut self, key: char, window: &mut Window, cx: &mut Context<Self>) {
        match menu::menu_by_mnemonic(key) {
            Some(index) => self.press_menu(index, window, cx),
            None => {
                if self.open_menu.take().is_some() {
                    self.menu_selected = None;
                    cx.notify();
                }
            }
        }
    }

    /// 点击 / 再次点击顶层菜单：展开、收起，或执行顶层命令按钮
    #[cfg(windows)]
    fn press_menu(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(spec) = menu::MENUS.get(index) else {
            return;
        };

        // 顶层命令按钮（无下拉）直接执行
        if spec.items.is_empty() {
            self.open_menu = None;
            self.dispatch(menu::Cmd::Settings, window, cx);
            return;
        }

        self.open_menu = if self.open_menu == Some(index) {
            None
        } else {
            Some(index)
        };
        self.menu_selected = None;
        // 让 Esc 等按键事件仍然送到本窗口
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    /// 窗口坐标落在哪个顶层菜单上
    #[cfg(windows)]
    fn menu_at(&self, position: Point<Pixels>) -> Option<usize> {
        let rects = self.menu_rects.borrow();
        menu::MENUS.iter().enumerate().find_map(|(index, _)| {
            let rect = rects.get(index)?;
            rect.contains(&position).then_some(index)
        })
    }

    /// 菜单展开时，鼠标滑到别的顶层菜单就跟着切换（同系统菜单行为）
    #[cfg(windows)]
    fn hover_menu_switch(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(index) = self.menu_at(position) else {
            return;
        };
        if self.open_menu == Some(index) || menu::MENUS[index].items.is_empty() {
            return;
        }
        self.open_menu = Some(index);
        self.menu_selected = None;
        cx.notify();
    }

    /// 上下键在可执行条目之间移动（跳过分隔线）
    #[cfg(windows)]
    fn move_menu_selection(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(spec) = self.open_menu.and_then(|index| menu::MENUS.get(index)) else {
            return;
        };
        let commands: Vec<usize> = spec
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| matches!(item, menu::MenuItem::Command { .. }))
            .map(|(index, _)| index)
            .collect();
        if commands.is_empty() {
            return;
        }

        let next = match self
            .menu_selected
            .and_then(|selected| commands.iter().position(|index| *index == selected))
        {
            Some(position) if forward => commands[(position + 1) % commands.len()],
            Some(position) => commands[(position + commands.len() - 1) % commands.len()],
            None if forward => commands[0],
            None => commands[commands.len() - 1],
        };
        self.menu_selected = Some(next);
        cx.notify();
    }

    /// 左右键切换顶层菜单（跳过纯命令按钮）
    #[cfg(windows)]
    fn switch_menu(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(current) = self.open_menu else {
            return;
        };
        let count = menu::MENUS.len();
        for step in 1..=count {
            let index = if forward {
                (current + step) % count
            } else {
                (current + count - step) % count
            };
            if !menu::MENUS[index].items.is_empty() {
                self.open_menu = Some(index);
                self.menu_selected = None;
                cx.notify();
                return;
            }
        }
    }

    /// 当前键盘选中的命令
    #[cfg(windows)]
    fn selected_cmd(&self) -> Option<menu::Cmd> {
        let spec = self.open_menu.and_then(|index| menu::MENUS.get(index))?;
        match spec.items.get(self.menu_selected?) {
            Some(menu::MenuItem::Command { cmd, .. }) => Some(*cmd),
            _ => None,
        }
    }

    /// 下拉菜单浮层：全窗遮罩负责关闭与切换，面板自绘
    ///
    /// 面板放在窗口内而不是单独的 popup 窗口：复用同一个 renderer / atlas，
    /// 也省掉定位、失焦、多显示器翻转那一整套系统语义。
    #[cfg(windows)]
    fn render_menu_layer(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let index = self.open_menu?;
        let spec = menu::MENUS.get(index)?;
        let palette = Palette::for_appearance(window.appearance());
        // 菜单展开时条目助记符始终可见（同系统菜单）
        let mnemonics = self.alt_held || self.open_menu.is_some();

        let window_width: f32 = window.viewport_size().width.into();
        let item_x = self
            .menu_rects
            .borrow()
            .get(index)
            .map(|rect| f32::from(rect.origin.x))
            .unwrap_or(8.0);
        // 贴右边缘时往左收，保证面板完整落在窗口里
        let panel_x = item_x.min((window_width - MENU_WIDTH - 8.0).max(8.0));

        let backdrop = div()
            .absolute()
            .inset_0()
            // 菜单打开期间挡住标题栏与内容区，避免误触窗口拖拽/查看器
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    // 点在顶层菜单上：切换/执行；点在别处：收起
                    if let Some(index) = this.menu_at(event.position) {
                        this.press_menu(index, window, cx);
                    } else if this.open_menu.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                this.hover_menu_switch(event.position, cx);
            }))
            .on_scroll(cx.listener(|this, _, _, cx| {
                if this.open_menu.take().is_some() {
                    cx.notify();
                }
            }));

        let mut panel = div()
            .absolute()
            .left(px(panel_x))
            .top(px(TITLEBAR_HEIGHT))
            .w(px(MENU_WIDTH))
            .flex()
            .flex_col()
            .p(px(4.0))
            .rounded(px(8.0))
            .bg(palette.menu_bg)
            .border_1()
            .border_color(palette.menu_border)
            .shadow(vec![
                BoxShadow::new(px(0.0), px(6.0), hsla(0.0, 0.0, 0.0, 0.22))
                    .blur_radius(px(18.0))
                    .spread_radius(px(-6.0)),
                BoxShadow::new(px(0.0), px(1.0), hsla(0.0, 0.0, 0.0, 0.14)).blur_radius(px(3.0)),
            ])
            // 面板空白处也不要漏到遮罩上（否则点分隔线会关掉菜单）
            .occlude();

        for (item_index, item) in spec.items.iter().enumerate() {
            match item {
                menu::MenuItem::Separator => {
                    panel = panel.child(
                        div()
                            .h(px(1.0))
                            .mx(px(8.0))
                            .my(px(4.0))
                            .bg(palette.menu_border),
                    );
                }
                menu::MenuItem::Command {
                    label,
                    shortcut,
                    mnemonic,
                    cmd,
                } => {
                    let cmd = *cmd;
                    let selected = self.menu_selected == Some(item_index);
                    let mut row = div()
                        .id(("menu-row", item_index))
                        .h(px(28.0))
                        .px(px(8.0))
                        .flex()
                        .items_center()
                        .rounded(px(4.0))
                        .text_size(px(12.0))
                        .text_color(palette.text)
                        .when(selected, |this| this.bg(palette.active))
                        .hover(move |style| style.bg(palette.hover))
                        // 悬停同步选中项：此时按回车执行的就是鼠标指向的命令
                        .on_mouse_move(cx.listener(move |this, _, _, cx| {
                            if this.menu_selected != Some(item_index) {
                                this.menu_selected = Some(item_index);
                                cx.notify();
                            }
                        }))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.dispatch(cmd, window, cx);
                        }))
                        .child(mnemonic_label(label, *mnemonic, palette.text, mnemonics));

                    if !shortcut.is_empty() {
                        row = row.child(div().flex_1()).child(
                            div()
                                .text_size(px(11.0))
                                .text_color(palette.text_muted)
                                .child(*shortcut),
                        );
                    }
                    panel = panel.child(row);
                }
            }
        }

        Some(
            div()
                .absolute()
                .inset_0()
                .child(backdrop)
                .child(panel)
                .into_any_element(),
        )
    }

    /// 首帧 / 最大化 / 全屏后重新压一遍窗口边框
    ///
    /// 窗口真正显示之后 DWM 会重建一次框架，创建时设的边框颜色会被还原，
    /// 表现就是标题栏顶部冒出一条 1px 亮线。
    #[cfg(windows)]
    fn refresh_window_frame(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.hwnd == 0 {
            return;
        }
        let hwnd = self.hwnd as *mut core::ffi::c_void;
        let border_rgb = Palette::for_appearance(window.appearance()).titlebar_rgb;
        window.defer(cx, move |_, _| win::apply_window_frame(hwnd, border_rgb));
    }

    /// 视口（标题栏以下的内容区）尺寸
    fn view_size(window: &Window) -> Size<gpui::Pixels> {
        let viewport = window.viewport_size();
        let height: f32 = viewport.height.into();
        size(viewport.width, px((height - TITLEBAR_HEIGHT).max(0.0)))
    }

    /// 当前图片的逻辑尺寸（无图时用 1×1 占位，避免除零）
    fn base_size(&self, window: &Window) -> Size<gpui::Pixels> {
        self.image
            .as_ref()
            .map(|image| image.logical_size(window.scale_factor()))
            .unwrap_or_else(|| size(px(1.0), px(1.0)))
    }

    /// 打开文件对话框（系统原生），选中后在后台解码
    #[cfg(windows)]
    fn open_file(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("打开图片".into()),
        });

        cx.spawn(async move |this, cx| {
            let picked = match receiver.await {
                Ok(Ok(paths)) => paths,
                _ => return,
            };
            let Some(path) = picked.and_then(|paths| paths.into_iter().next()) else {
                return;
            };
            this.update_in(cx, |this, window, cx| this.open_path(path, window, cx))
                .ok();
        })
        .detach();
    }

    /// 后台分级解码并替换当前图片
    fn open_path(&mut self, path: PathBuf, window: &Window, cx: &mut Context<Self>) {
        self.load_id = self.load_id.wrapping_add(1);
        let load_id = self.load_id;
        self.loading = true;
        self.error = None;

        // 首帧按视口宽度选档：能覆盖视口即可，不必解全图
        let view_width: f32 = Self::view_size(window).width.into();
        let need_px = (view_width * window.scale_factor()).round().max(1.0) as u32;
        eprintln!("[SPIKE] loading {} (need {need_px}px)", path.display());
        cx.notify();

        cx.spawn(async move |this, cx| {
            let task_path = path.clone();
            let decode_path = task_path.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let started = Instant::now();
                    let meta = decode::probe(&decode_path)?;
                    let preview = decode::load_preview(&decode_path, &meta, need_px)?;
                    Ok::<_, String>((meta, preview, started.elapsed()))
                })
                .await;

            this.update(cx, |this, cx| {
                // 已被更新的请求取代
                if this.load_id != load_id {
                    return;
                }
                this.loading = false;
                match result {
                    Ok((meta, preview, elapsed)) => {
                        let display = meta.display_size();
                        eprintln!(
                            "[SPIKE] {} → {}x{} (1/{}) via {} in {} ms, texture {}x{} {:.2} MB",
                            task_path
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("?"),
                            display.0,
                            display.1,
                            preview.denom,
                            preview.backend,
                            elapsed.as_millis(),
                            preview.width(),
                            preview.height(),
                            preview.byte_len() as f64 / 1_048_576.0
                        );
                        this.image = Some(viewer::from_preview(task_path, display, preview));
                        this.view = viewer::ViewState::default();
                        this.error = None;
                    }
                    Err(message) => this.error = Some(message),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// 切换缩放方式
    fn set_mode(&mut self, mode: viewer::ViewMode, window: &Window) {
        if self.image.is_none() {
            return;
        }
        let view = Self::view_size(window);
        let base = self.base_size(window);
        self.view.set_mode(mode, view, base);
    }

    /// 以视口中心为锚点缩放
    fn zoom_step(&mut self, factor: f32, window: &Window) {
        if self.image.is_none() {
            return;
        }
        let view = Self::view_size(window);
        let base = self.base_size(window);
        let view_w: f32 = view.width.into();
        let view_h: f32 = view.height.into();
        let cursor = point(px(view_w / 2.0), px(view_h / 2.0));
        self.view.zoom_at(factor, cursor, view, base);
    }

    /// 键盘快捷键
    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let key = keystroke.key.as_str();
        let modifiers = keystroke.modifiers;

        // Alt+助记符直达顶层菜单（AltGr 是 Ctrl+Alt，必须排除）
        if modifiers.alt && !modifiers.control && !modifiers.platform {
            if let Some(mnemonic) = single_char(key) {
                cx.stop_propagation();
                self.press_mnemonic(mnemonic, window, cx);
                return;
            }
        }

        // 菜单打开时先由菜单消费键盘：Esc 关闭，方向键移动/切换，回车执行
        if self.open_menu.is_some() {
            match key {
                "escape" => {
                    self.open_menu = None;
                    self.menu_selected = None;
                    cx.notify();
                    return;
                }
                "down" => {
                    self.move_menu_selection(true, cx);
                    return;
                }
                "up" => {
                    self.move_menu_selection(false, cx);
                    return;
                }
                "right" => {
                    self.switch_menu(true, cx);
                    return;
                }
                "left" => {
                    self.switch_menu(false, cx);
                    return;
                }
                "enter" => {
                    if let Some(cmd) = self.selected_cmd() {
                        self.dispatch(cmd, window, cx);
                    }
                    return;
                }
                // 菜单展开时直接按助记符执行（原生行为），否则收起后按普通快捷键处理
                _ => {
                    let cmd = single_char(key).and_then(|mnemonic| {
                        let index = self.open_menu?;
                        menu::cmd_by_mnemonic(index, mnemonic)
                    });
                    if let Some(cmd) = cmd {
                        self.dispatch(cmd, window, cx);
                        return;
                    }
                    self.open_menu = None;
                    self.menu_selected = None;
                    cx.notify();
                }
            }
        }

        // 单键快捷键只在无修饰键时生效，避免 Alt+数字 等被误当成缩放
        let plain = !modifiers.control && !modifiers.alt && !modifiers.platform;

        if key.eq_ignore_ascii_case("escape") {
            if window.is_fullscreen() {
                window.toggle_fullscreen();
            }
            return;
        }

        if modifiers.control && !modifiers.alt && key.eq_ignore_ascii_case("o") {
            #[cfg(windows)]
            self.open_file(cx);
            return;
        }

        if plain && key.eq_ignore_ascii_case("f11") {
            window.toggle_fullscreen();
            return;
        }

        if self.image.is_none() || !plain {
            return;
        }

        match key {
            "0" => self.set_mode(viewer::ViewMode::Fit, window),
            "1" => self.set_mode(viewer::ViewMode::Original, window),
            "2" => self.set_mode(viewer::ViewMode::Fill, window),
            "+" | "=" => self.zoom_step(1.25, window),
            "-" | "_" => self.zoom_step(0.8, window),
            _ => return,
        }
        cx.notify();
    }

    /// 菜单命令落地（后续这里会长成真正的 app action）
    #[cfg(windows)]
    fn dispatch(&mut self, cmd: menu::Cmd, window: &mut Window, cx: &mut Context<Self>) {
        use menu::Cmd;

        // 任何命令都会收起下拉
        self.open_menu = None;
        self.menu_selected = None;

        match cmd {
            Cmd::Exit => {
                window.defer(cx, |window, _| window.remove_window());
                return;
            }
            Cmd::Open => {
                self.open_file(cx);
                cx.notify();
                return;
            }
            Cmd::Fullscreen => {
                window.toggle_fullscreen();
            }
            Cmd::Fit => self.set_mode(viewer::ViewMode::Fit, window),
            Cmd::Original => self.set_mode(viewer::ViewMode::Original, window),
            Cmd::Fill => self.set_mode(viewer::ViewMode::Fill, window),
            _ => {}
        }

        eprintln!("[SPIKE] command {cmd:?}");
        cx.notify();
    }

    /// 内容区：图片 + 缩放/平移交互
    fn render_viewer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = Palette::for_appearance(window.appearance());
        let view = Self::view_size(window);
        let scale_factor = window.scale_factor();

        let mut area = div()
            .id("viewer")
            .relative()
            .flex_1()
            .overflow_hidden()
            .bg(palette.content_bg)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, _| {
                    this.drag_last = Some(event.position);
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if !event.dragging() {
                    this.drag_last = None;
                    return;
                }
                let Some(last) = this.drag_last else {
                    this.drag_last = Some(event.position);
                    return;
                };
                let delta = event.position - last;
                this.drag_last = Some(event.position);

                if this.image.is_some() {
                    let view = Self::view_size(window);
                    let base = this.base_size(window);
                    this.view.pan(delta, view, base);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag_last = None),
            )
            .on_scroll(cx.listener(|this, event: &ScrollEvent, window, cx| {
                if this.image.is_none() {
                    return;
                }
                let view = Self::view_size(window);
                let base = this.base_size(window);
                let dy = match event.delta {
                    ScrollDelta::Pixels(delta) => delta.y.into(),
                    // 一行约 30 逻辑像素，一格滚轮（约 3 行）≈ 25%
                    ScrollDelta::Lines(delta) => delta.y * 30.0,
                };
                if dy == 0.0 {
                    return;
                }
                // 每 1 逻辑像素滚动放大约 0.25%
                let factor = (dy * 0.0025).exp();
                let cursor = event.position - point(px(0.0), px(TITLEBAR_HEIGHT));
                this.view.zoom_at(factor, cursor, view, base);
                cx.notify();
            }));

        if let Some(image) = &self.image {
            area = area
                .child(self.render_image_layer(image, view, scale_factor))
                .child(self.render_status_badge(image, view, scale_factor, &palette));
        } else {
            area = area.child(self.render_placeholder(&palette));
        }

        area.into_any_element()
    }

    /// 图片层：按当前缩放/平移放置
    fn render_image_layer(
        &self,
        image: &viewer::LoadedImage,
        view: Size<gpui::Pixels>,
        scale_factor: f32,
    ) -> AnyElement {
        let base = image.logical_size(scale_factor);
        let bounds = self.view.image_bounds(view, base);

        div()
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height)
            .child(
                img(image.image.clone())
                    .object_fit(ObjectFit::Fill)
                    .size_full(),
            )
            .into_any_element()
    }

    /// 左下角状态：尺寸与缩放百分比
    fn render_status_badge(
        &self,
        image: &viewer::LoadedImage,
        view: Size<gpui::Pixels>,
        scale_factor: f32,
        palette: &Palette,
    ) -> AnyElement {
        let base = image.logical_size(scale_factor);
        let percent = self.view.percent(view, base);
        let label = format!(
            "{} × {}   {:.0}%",
            image.pixel_size.0, image.pixel_size.1, percent
        );

        div()
            .absolute()
            .left(px(12.0))
            .bottom(px(12.0))
            .px(px(8.0))
            .py(px(4.0))
            .rounded(px(6.0))
            .bg(palette.card_bg)
            .border_1()
            .border_color(palette.card_border)
            .text_size(px(11.0))
            .text_color(palette.text_muted)
            .child(label)
            .into_any_element()
    }

    /// 无图 / 加载中 / 出错时的占位
    fn render_placeholder(&self, palette: &Palette) -> AnyElement {
        let (title, hint) = if self.loading {
            ("加载中…".to_string(), String::new())
        } else if let Some(error) = &self.error {
            (error.clone(), String::new())
        } else {
            (
                "拖入图片，或按 Ctrl+O 打开".to_string(),
                "支持 JPEG / PNG / WebP / GIF / BMP / TIFF".to_string(),
            )
        };
        let title_color = if self.error.is_some() {
            hsla(0.0, 0.72, 0.60, 1.0)
        } else {
            palette.text
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(title_color)
                    .child(title),
            )
            .when(!hint.is_empty(), |this| {
                this.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(palette.text_muted)
                        .child(hint),
                )
            })
            .into_any_element()
    }
}

impl Render for Spike {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 启动到首帧的耗时（gpui 侧冷启动基线）
        let first_frame = !self.first_frame_logged;
        if first_frame {
            self.first_frame_logged = true;
            eprintln!(
                "[SPIKE] first frame after {} ms",
                self.started_at.elapsed().as_millis()
            );
        }

        // 窗口显示、最大化、全屏都会重建 DWM 框架，重新把边框画成标题栏色
        let window_state = (window.is_maximized(), window.is_fullscreen());
        if first_frame || self.window_state != window_state {
            self.window_state = window_state;
            #[cfg(windows)]
            self.refresh_window_frame(window, cx);
        }

        // 系统深浅色切换后换一套配色，并把 DWM 边框色同步到新的标题栏底色
        if self._appearance_sub.is_none() {
            self._appearance_sub = Some(cx.observe_window_appearance(window, |this, window, cx| {
                #[cfg(windows)]
                if this.hwnd != 0 {
                    let border_rgb = Palette::for_appearance(window.appearance()).titlebar_rgb;
                    win::apply_window_frame(this.hwnd as *mut core::ffi::c_void, border_rgb);
                }
                cx.notify();
            }));
        }

        // 窗口从隐藏变为可见时，DWM 会把边框颜色还原成默认值
        if self._visibility_sub.is_none() {
            self._visibility_sub =
                Some(cx.observe_window_visibility(window, |this, _, window, _| {
                    #[cfg(windows)]
                    if this.hwnd != 0 {
                        let border_rgb = Palette::for_appearance(window.appearance()).titlebar_rgb;
                        win::apply_window_frame(this.hwnd as *mut core::ffi::c_void, border_rgb);
                    }
                }));
        }

        // 窗口失焦时收起菜单并清掉 Alt 状态（系统菜单同行为）
        if self._activation_sub.is_none() {
            self._activation_sub =
                Some(cx.observe_window_activation(window, |this, window, cx| {
                    if window.is_window_active() {
                        return;
                    }
                    // Alt+Tab 时松开 Alt 收不到事件，必须自己复位下划线
                    let alt_reset = std::mem::take(&mut this.alt_held);
                    let menu_closed = this.open_menu.take().is_some();
                    if alt_reset || menu_closed {
                        cx.notify();
                    }
                }));
        }

        let titlebar = self.render_titlebar(window, cx);
        let focus_handle = self.focus_handle.clone();
        let menu_layer = self.render_menu_layer(window, cx);

        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .track_focus(&focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key(event, window, cx);
            }))
            // 助记符下划线跟着 Alt 的按下/松开走
            .on_modifiers_changed(cx.listener(
                |this, event: &ModifiersChangedEvent, _, cx| {
                    if this.alt_held != event.modifiers.alt {
                        this.alt_held = event.modifiers.alt;
                        cx.notify();
                    }
                },
            ))
            .child(titlebar)
            .child(self.render_viewer(window, cx))
            .when_some(menu_layer, |this, layer| this.child(layer))
    }
}

/// 取窗口原生句柄（Windows）
fn native_handle(window: &Window) -> isize {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        // 显式走 trait：gpui 自身也有同名的 `window_handle()` 返回 AnyWindowHandle
        if let Ok(RawWindowHandle::Win32(win32)) =
            HasWindowHandle::window_handle(window).map(|handle| handle.as_raw())
        {
            return win32.hwnd.get();
        }
        0
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        0
    }
}

fn main() {
    let started_at = Instant::now();
    // 实色方案：标题栏与内容区各自不透明，靠明度差区分（同 VS Code / Windows Terminal）。
    // `Opaque` 下渲染器用白色清屏，但整块客户区都被我们的实色覆盖，不会露出清屏色。
    let backdrop = WindowBackgroundAppearance::Opaque;
    let initial_path = std::env::args_os().nth(1).map(PathBuf::from);

    application().run(move |cx: &mut App| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(960.0), px(640.0)),
                cx,
            ))),
            // 自绘标题栏：菜单与窗口按钮都放进标题栏，避免多占一行
            titlebar: Some(TitlebarOptions {
                title: Some("FastView".into()),
                appears_transparent: true,
                traffic_light_position: None,
            }),
            window_background: backdrop,
            window_min_size: Some(size(px(420.0), px(320.0))),
            ..Default::default()
        };

        cx.open_window(options, move |window, cx| {
            let hwnd = native_handle(window);
            #[cfg(windows)]
            if hwnd != 0 {
                // 把 DWM 的 1px 边框画成标题栏底色，避免自绘标题栏顶部出现亮线
                let border_rgb = Palette::for_appearance(window.appearance()).titlebar_rgb;
                win::apply_window_frame(hwnd as *mut core::ffi::c_void, border_rgb);
            }
            let view = cx.new(|cx| {
                let focus_handle = cx.focus_handle();
                Spike {
                    hwnd,
                    started_at,
                    first_frame_logged: false,
                    _appearance_sub: None,
                    _visibility_sub: None,
                    image: None,
                    view: viewer::ViewState::default(),
                    loading: false,
                    error: None,
                    load_id: 0,
                    drag_last: None,
                    focus_handle,
                    window_state: (false, false),
                    open_menu: None,
                    menu_selected: None,
                    menu_rects: Rc::new(RefCell::new(Vec::new())),
                    alt_held: false,
                    _activation_sub: None,
                }
            });

            // 让内容区直接接收键盘快捷键
            let handle = view.read(cx).focus_handle.clone();
            window.focus(&handle, cx);

            // 命令行传入的图片在首帧后后台解码，避免阻塞启动
            if let Some(path) = initial_path.clone() {
                let weak = view.downgrade();
                window.defer(cx, move |window, cx| {
                    weak.update(cx, |this, cx| this.open_path(path, window, cx))
                        .ok();
                });
            }

            view
        })
        .expect("failed to open window");

        cx.activate(true);
    });
}
