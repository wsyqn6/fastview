# 🖼️ FastView Agent 配置

## 🎯 目标
**极轻 + 极快 + 好看**的图片查看器：流畅度与占用对标 HoneyView，交互与视觉对标现代 Windows 11 原生应用。

优先级（冲突时按此顺序取舍）：

1. **看图流畅** —— 大图缩放/平移不卡顿、不等待
2. **占用低** —— 内存与 CPU 常驻开销小，后台不空转
3. **UI 精美** —— 系统材质、克制动效、现代排版
4. **二进制体积** —— 不刻意优化，`opt-level`/`strip` 等够用即可

方向性指标（用来判断是否退化，不是硬性验收）：

| 维度 | 参考值 |
|------|--------|
| ⚡ 冷启动 | < 300 ms（进程启动 → 首帧） |
| 💾 常驻内存 | < 100 MB（打开 24MP 图片后的稳定值） |
| 🖼️ 大图打开 | < 500 ms（24MP JPEG 首帧可见，其余分块后台补齐） |
| 🎞️ 缩放/平移 | 60 fps+，UI 线程不做解码/拷贝 |

原则：**不为了省几 MB 牺牲流畅度和观感**，也不为了多一个功能牺牲常驻开销。

## 🛠️ 技术栈
| 组件 | 选型 |
|------|------|
| 🎨 GUI（主线） | eframe/egui 0.34 |
| 🎨 GUI（gpui 线） | gpui-unofficial 1.28.1 + gpui-platform-gpui-unofficial（独立 crate） |
| 🖼️ 图片 | image 0.25 |
| 📂 对话框 | rfd 0.16（egui 线）/ gpui_windows 原生 dialog（gpui 线） |
| ⚙️ 配置 | serde + dirs |

## 🌿 分支策略
- `main`：egui 主线，**暂时冻结、不迭代**，只作为功能参考。
- `explore/gpui`：gpui 重写线，**与 egui 无任何代码共享**，只保持功能对齐；实验代码在 `spike/gpui-spike`（独立 workspace，不参与主构建）。
- gpui 线**不必照搬 main**：main 的设计与实现不一定合理、简洁、高效。
  允许在功能一致的前提下重新设计，选择更简单、更正确、更高性能的实现。
  判断标准是目标（流畅、低占用、UI 精美），不是与 main 代码形似。
- 两条线各自演进，不互相 cherry-pick；公共能力（解码、缓存、配置格式）可各自实现，不强行抽库。

## 🪟 平台策略
- **Windows 优先**：新能力、性能优化、交互细节都以 Win11 体验为准。
- macOS / Linux 只保留基本可用（打开、翻页、缩放、退出），不为边缘平台牺牲 Windows 方案。

## 🎛️ Windows 平台能力（实测结论，2026-11）
- gpui 窗口走 DirectComposition（`WS_EX_NOREDIRECTIONBITMAP`），此模式下：
  - ✅ Mica / MicaAlt / 亚克力 / 透明：`WindowBackgroundAppearance` 真正生效（DWM 材质）；
  - ❌ Win32 HMENU 菜单条（`SetMenu`）**装得上但画不出来**，客户区会被挤压而已；
  - ✅ `TrackPopupMenuEx` 原生下拉菜单正常（独立 popup 窗口，不受限制）。
- 关掉 DComp（`GPUI_DISABLE_DIRECT_COMPOSITION=1`）菜单条恢复，但 Mica 变纯黑 —— **材质与系统菜单条互斥**。
- `WindowBackgroundAppearance::Transparent` 在 Windows 上透出的是**桌面壁纸**，不是下层窗口内容。
- egui 侧可用 `eframe::Frame` 的 `HasWindowHandle` 直接取 HWND 做原生扩展。

## 🪟 标题栏规范（自绘 + 保留系统行为）
菜单与窗口按钮**放在标题栏内**，不额外占一行（同 VS Code / 现代 Win11 应用）。

- 窗口用 `TitlebarOptions::appears_transparent = true` 自绘，菜单条与按钮都由 gpui 绘制。
- **原生行为必须保留**，用 `window_control_area` 把命中区域交给系统：
  - 标题栏空白区 / 左留白 → `WindowControlArea::Drag`（`HTCAPTION`）：拖动、拖动到屏幕边缘分屏、双击最大化全部由 `DefWindowProc` 处理；
  - 最小化 / 最大化 / 关闭 → `Min` / `Max` / `Close`（`HTMINBUTTON` / `HTMAXBUTTON` / `HTCLOSE`）：系统负责点击行为、最小化/还原动画与悬停「贴靠布局」；
  - 不要再自己实现 `start_window_move` + `SetWindowPos` 的拖拽，会丢掉 DWM 动画与分屏。
- 命中盒需要元素有显式 `.id()`，否则 `window_control_area` 不生效。
- 窗口控制图标**全部用几何图形自绘**（横线 / 方框 / 双框 / 两条对角线），统一 `Path` 填充 + 1px 线宽，不用字体：`Segoe Fluent Icons` 在 gpui 里匹配不到，字形会渲染成方框；文本 `×` 与几何线条粗细不一致。
- 标题栏与主体都**不透明实色**，只靠底色明度区分，不加 1px 分隔线（参考 Windows Terminal / VS Code）；主体必须完全不透明（图片取色环境不能受桌面影响），不使用 Mica/亚克力材质。
- 菜单入口自绘、下拉用 `TrackPopupMenuEx`，命令回到 app 层统一分发。

## ⌨️ 快捷键
- `←/→` 上一张/下一张 | `+/-` 缩放 | `0/1/2` 适应/原始/填充
- `R`/`Shift+R` 旋转 | `F` 全屏 | `V` 无边框 | `S` 状态栏 | `T` 缩略图
- `Space` 按住拖动 | `H`/`?` 帮助

## 📜 编码规范

### 核心原则
- **简洁优先**: 避免过度设计，实现最直接的解决方案
- **高效实现**: 优先使用 `rayon` 并行处理，避免阻塞主线程
- **API 最新**: 定期升级依赖版本，使用最新稳定 API
- **错误处理**: 禁止裸 `unwrap()`，使用 `?` 或 `ok_or_else()`

### 代码风格
- 函数不超过 50 行，复杂逻辑拆分
- 使用 `camelCase` 命名（Rust 风格）
- 宏定义放在 `utils.rs` 统一管理
- 公共 API 添加文档注释

### 性能规范
- 图片加载使用分块渲染（`tile_renderer`）
- 缓存策略：LRU + 纹理池复用，上限按内存预算而非条目数
- 异步加载，不阻塞 UI 线程；主线程禁止整图解码/缩放
- 每帧工作量与图片分辨率解耦（只处理可见块）
- 空闲时不刷帧，动效结束即停

### 安全规范
- 路径操作使用 `PathBuf`，验证文件存在性
- 网络请求设置合理超时
- 锁使用 `lock_or_recover()` 避免中毒

## 🚧 功能边界
✅ 查看、缩放、旋转、缓存、中英切换  
❌ 编辑、管理、云同步

## 🎨 UI 要点
- 现代、克制、有质感：Mica/毛玻璃背景、圆角、半透明浮层
- 间距只用 4/8/12 的倍数，字号取整数，图标与文字基线对齐
- 状态栏居中悬浮、超时淡出；全屏隐藏菜单与状态栏
- 动效短促（≤150ms）、可中断，不做炫技动画
- 文案通过 `TextKey` 枚举管理，跟随系统深浅色

## 💾 配置路径
`%APPDATA%\fastview\settings.json`

## 📦 构建命令
```bash
# 开发构建
cargo build

# 运行（主线）
cargo run --release -- <图片路径>

# gpui 实验（可直接传图片路径）
cd spike/gpui-spike && cargo run --release -- <图片路径>
```

环境变量（仅排查用）：`GPUI_DISABLE_DIRECT_COMPOSITION=1` 关闭 DComp，用于对比菜单条行为。

> 注意：spike 是带窗口的 GUI 程序，除用户明确要求外不要自动启动，避免打断用户工作。
