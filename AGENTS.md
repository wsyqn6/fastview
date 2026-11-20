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
| 🖼️ 图片 | image 0.25（全质量解码）+ WIC（缩放预览 / EXIF / 尺寸） |
| 📂 对话框 | rfd 0.16（egui 线）/ gpui_windows 原生 dialog（gpui 线） |
| ⚙️ 配置 | serde + dirs |

## 🖼️ 图像管道（分级解码，实测 2026-11）

egui + `image` 旧链路的三个病灶，gpui 线不再重复：

- **假分块**：`decode_tile` 每块把整图完整解码再裁剪，24 块 = 24 次全解码，比不分块更慢更费内存。
- **无渐进**：`image` 的 JPEG 后端没有 DCT 缩放，只能「全解码 → resize」，低分辨率预览一分延迟都省不掉。
- **内存失控**：`DecodedImage` 强制 RGBA8 常驻，`to_rgba8` 是白花的拷贝（24MP ≈ +46 ms / +91 MB）；LRU 按条目数而非字节淘汰。

**gpui 硬约束（源码实测）**：`gpui-windows` 的 atlas 把整图当成**一个** tile 上传，`MAX_ATLAS_SIZE = 16384`，不降采样、`MipLevels: 1`：

- 单边 > 16384 → 分配失败，图片直接画不出来；
- 8000×3000 就是一张 96 MB 的 D3D11 纹理；
- 无 mipmap，缩小显示只能一次双线性，质量劣化。
→ **分块与多级 LOD 必须自己做，不是可选项。**

分级方案（8000×3000 实测）：

| 层 | 来源 | 耗时 / 占用 | 触发时机 |
|---|---|---|---|
| L0 首帧 | WIC DCT 1/8 | 18 ms / 1.4 MB | 切图立即 |
| L1 静止 | WIC DCT 1/2ⁿ 覆盖视口 | 26 ms (1/4) / 5.7 MB | 首帧之后紧接着 |
| L2 放大 | `image` 全解码一次 → 按行写块 | 71~105 ms / 91 MB 峰值 | zoom > 100% |
| 缩略图条 | `IShellItemImageFactory` | 命中 3~10 ms（首次 50~165 ms） | 目录浏览 |

实现约束：

- **L1 档位是纯算术**：取最小 `n` 使 `w / 2ⁿ ≥ 视口宽`；WIC 只支持 1/2ⁿ，没有中间档，别指望精确值。
- **L2 单线程串行**：整图解一次、按行写块、解完即释放；禁止并行多个全解码，峰值内存会翻倍。
- **块尺寸 1024 且边缘块补齐**：gpui atlas 是 bucketed allocator，尺寸统一才能复用同一张纹理页。
- **取消靠 generation id**：解码本身不可中断，切图时丢弃过期结果即可；并发上限 2。
- **纹理必须显式回收**：gpui **不会**随 `RenderImage` 销毁清除 atlas（内无 drop 钩子）。纹理所有权统一交给 `cache.rs`，**淘汰时**才 `window.drop_image()`；任何新增的贴图路径都要走 `present()`，否则不是泄漏就是悬空。
- **WIC 全尺寸解码比 zune-jpeg 慢**（71 vs 58 ms），因此 L1 走 WIC、L2 仍走 `image`。
- **WIC 不做色彩管理**：32bppBGRA 输出是原始像素，ICC 需显式 `IWICColorTransform`（暂不做）。
- **EXIF 方向**：WIC 不自动应用，`GetSize` 返回存储尺寸，方向需自己读 metadata 后旋转。
- **首帧不使用 shell 缩略图**：冷进程 `SHCreateItemFromParsingName` 要 35~40 ms，且首次生成要 50~165 ms；它只用在缩略图条。

测量工具：`spike/wic-probe`（无窗口 CLI），`cargo run --release -- <图片路径>`。

### 实施批次

1. ✅ **首帧分级解码**：`spike/gpui-spike/src/decode`（`probe` → `pick_denom` → WIC 缩放 → 方向归一 + `image` 兜底）。8000×3000 实测 46 ms / 5.72 MB，20000×2000 实测 41 ms / 2.38 MB。
2. ✅ **档位补齐**：`schedule_sharpen` 在 render 里检测，按 generation 去重；分档允许 5% 插值放大，避免临界点跳档。
3. ✅ **缩略图条 + 目录翻页**：`shell.rs` 取系统 thumbcache，`folder.rs` 扫目录并自然排序，`←/→` 循环翻页。
4. ✅ **纹理缓存 + 预取**：`cache.rs` 按字节 LRU 缓存**已上传的纹理**（预算 192 MB，key 为「图片 + 档位」，同时存 probe 结果）。切回看过的图零解码，前后各预取 1 张。
5. ⏸️ **L2 分块（暂缓）**：整图解码一次 → 按行写入 1024 块 → 可见块优先上传。
   触发条件：拿到 24MP+ 真实样张，或「原始尺寸」下的显存占用（24MP ≈ 91 MB、48MP ≈ 183 MB）被判定不可接受。
   当前 1/1 档已能覆盖 4K 屏，且没有可验证的样本，先不做。

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
  - ✅ `TrackPopupMenuEx` 原生下拉正常（独立 popup 窗口），但它会跑自己的模态消息循环，阻塞 gpui；
  - ❌ `WindowKind::AnchoredPopup`：gpui 1.28 的 Windows 后端**未实现**，直接返回 `PopupNotSupportedError`。
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
- 标题栏与主体共用同一套半透明底色，只靠底色明度区分，不加 1px 分隔线（参考 Windows Terminal / VS Code）。
- 窗口材质与不透明度是**两个独立设置**：材质只决定是否请求系统 backdrop；不透明度只决定自绘背景层 alpha。
  - 默认亚克力 + 65% 不透明；100% 时窗口用 `Opaque`，恢复 subpixel AA，材质视觉上不可见。
  - Win11 可选 `亚克力 / 云母 / 无`，走 `DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE)`：亚克力 = `3`，云母 = `2`。
  - Win10 没有系统 backdrop，设置里隐藏「云母」，亚克力回退到 SWCA `ACCENT_ENABLE_ACRYLICBLURBEHIND`；「无模糊 + <100%」只透明不模糊。
  - v1 配置里的 `material=none` 迁移为 v2 的 `material=none + opacity=100`，保持原来实色行为。
- 边框根因：gpui 在开窗时按 `border_offset` 固定布局尺寸；之后再改 `WM_NCCALCSIZE` 会让左右下多出一圈 gpui 没绘制的透明像素。解决顺序：`boot.rs` 创建窗口时设 `is_resizable=false`，让 gpui 量到 `border_offset=0`；`win.rs::install_window_hooks` 随后加回 `WS_THICKFRAME|WS_MAXIMIZEBOX`，再用 `WM_NCCALCSIZE` 把客户区扩到整窗、**必须保存进入时的 `NCCALCSIZE_PARAMS.rgrc[0]`（新窗口矩形）再写回，`rgrc[2]` 不是窗口矩形**；`WM_NCHITTEST` 补左右下缩放命中。Win11 用 `DWMWA_COLOR_NONE`；Win10 用 `DWMWA_NCRENDERING_POLICY=DWMNCRP_DISABLED` 去 DWM shadow。
- 菜单入口与下拉**都在窗口内自绘**：下拉是一层全窗遮罩（负责关闭/跟随切换）+ 面板，复用同一套配色、字号与圆角；不用 `SetMenu`，也不用 `TrackPopupMenuEx`（模态循环会阻断 gpui 事件）。
- **拖动跟手**：不要拦截移动循环的重绘。DComp 窗口（`WS_EX_NOREDIRECTIONBITMAP`）没有重定向位图，画面内容跟随窗口位置**依赖每帧 present**；压低重绘会变成「边框在动、画面追不上」。
  - 透明材质在拖动时卡顿的根因是 Win10 SWCA 亚克力每帧重采样背景；`win.rs` 用 `SetWindowSubclass` 只接管 `WM_ENTERSIZEMOVE` / `WM_EXITSIZEMOVE`：进入时把模糊降为 `ACCENT_ENABLE_BLURBEHIND`（低开销），退出时恢复，窗口自身 alpha 始终不变。
  - 钩子**不动** `WM_TIMER` / `WM_PAINT`；Win11 的 DWM 材质同理临时切到 `DWMSBT_NONE` 再恢复。
  - `ACTIVE_BACKDROP` / `LEGACY_ACCENT_STATE` 存**期望材质**，`SIZE_MOVE_ACTIVE` 存当前是否处于移动循环；最大化/还原触发 `apply_window_style` 时也只会应用低开销材质，避免覆盖拖动降级。

## ⌨️ 快捷键
- `←/→` 上一张/下一张 | `+/-` 缩放 | `0/1/2` 适应/原始/填充
- `R`/`Shift+R` 旋转 | `F` 全屏 | `V` 无边框 | `S` 状态栏 | `T` 缩略图
- `Space` 按住拖动 | `H`/`?` 帮助
- `Alt+F/V/S/H` 展开菜单（AltGr 不算）；菜单展开后直接按 `O/X/0/1/2/U/K/A` 执行，`↑↓←→` 移动、`Enter` 执行、`Esc` 收起
- 文案常显 `文件(F)`，下划线**只在按住 Alt 或菜单展开时**出现（同系统菜单）；Alt 状态来自元素级 `on_modifiers_changed`，失焦时手动复位（Alt+Tab 收不到 keyup）；下划线颜色必须显式指定，`UnderlineStyle.color` 为 `None` 时是透明色
- gpui spike：`Ctrl+,` 开关设置面板（主题 / 材质 / 不透明度），`Esc` 关闭；面板展开时其余快捷键不落到查看器

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
- 涉及图片加载的核心逻辑必须慎之又慎，保持简洁、高效、高可维护；改动加载/解码/缓存/选档后用测试与日志验证
- 图片走分级管道：L0/L1 快速预览 + L2 分块补齐（见「图像管道」）
- 解码并发上限 2，L2 全解码必须串行；`rayon` 只用于目录扫描等真可并行的任务
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
- `V` 无边框：隐藏标题栏 / 菜单 / 状态栏但仍留在任务栏；底部给「按 V 恢复界面」提示，顶部 8px 窄条保留窗口拖动
- 状态栏一行只放「序号/总数 · 尺寸 · 旋转角 · 缩放模式 · 百分比」：不重复标题栏已显示的文件名；旋转角与缩放模式只在非默认状态（0° / 非「适应」）时出现
- 动效短促（≤150ms）、可中断，不做炫技动画
- 文案通过 `TextKey` 枚举管理，跟随系统深浅色

## 🧭 后续功能（已确认方向）
- **图片信息窗口**（`I`）：EXIF（相机/镜头/ISO/快门/光圈/拍摄日期）、ICC/色彩空间、文件大小、格式等需要解码或 metadata 的数据。这类信息不进状态栏，单独设计窗口/功能。

## 💾 配置路径
- egui 主线：`%APPDATA%\fastview\settings.json`
- gpui spike：`%APPDATA%\fastview\gpui-settings.json`（独立文件，避免两条线互相覆盖字段）

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
