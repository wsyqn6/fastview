# FastView Agent Configuration

## 项目定位
**专注图片查看**的轻量工具，追求：
- 🚀 **启动快**：< 1秒
- 💾 **体积小**：~5MB（发布版）
- 🖼️ **格式全**：JPEG/PNG/GIF/WebP/BMP/TIFF/ICO/AVIF
- ✨ **UI现代**：简洁流畅的即时界面

## 技术选型
**原则**：最小依赖、零运行时、原生性能

| 组件 | 选型 | 理由 |
|------|------|------|
| GUI | Eframe (egui) 0.32 | 即时模式，无Webview，体积小 |
| 图片解码 | image-rs 0.25 | 纯Rust，支持主流格式 |
| 文件对话框 | rfd 0.16 | 调用系统原生对话框 |
| 配置 | serde + dirs | 最小化依赖 |

## 项目结构
```
src/
├── main.rs    # 入口（模块声明 + 启动）
├── types.rs   # 类型定义
├── fonts.rs   # 字体加载
└── app.rs     # 应用逻辑
```

## 常用命令
```bash
cargo run          # 开发运行
cargo run --release  # 发布运行
cargo check        # 检查编译
```

## 快捷键
| 功能 | 按键 | 功能 | 按键 |
|------|------|------|------|
| 上一张/下一张 | ←/→ | 适应/原始/填充 | 0/1/2 |
| 放大/缩小 | +/- | 顺时针/逆时针旋转 | R/Shift+R |
| 全屏 | F | 拖动模式 | Space(按住) |
| 快捷键帮助 | H | 退出/关闭 | Esc |

## 核心功能（专注查看）
✅ 快速加载 | ✅ 流畅缩放 | ✅ 平滑旋转  
✅ 键盘导航 | ✅ 拖动浏览 | ✅ 缩略图预览  
✅ 自动缓存 | ✅ 中英界面 | ✅ 配置持久化

**不做**：编辑、管理、云同步等复杂功能

## Skills 优先级
1. **brainstorming** - 功能设计前
2. **systematic-debugging** - 排查 bug
3. **frontend-design** - UI 优化
4. **writing-plans** - 复杂任务规划

## 开发约定
- 提交：Conventional Commits (`feat:`, `fix:`, `refactor:`...)
- 分支：PR → `main`
- 模块：types(类型) / fonts(字体) / app(逻辑) / main(入口)

## UI 设计规范

### 设计原则
- **轻量简洁**：符合"体积小、启动快"定位，避免过度装饰
- **视觉一致**：所有 UI 组件保持统一的设计语言
- **即时响应**：无动画过渡，点击即响应
- **跨平台一致**：完全自绘，不依赖系统原生控件

### 颜色系统
使用 egui 主题色变量，确保深浅主题适配：
- **背景色**：`ctx.style().visuals.panel_fill`
- **毛玻璃效果**：`panel_fill.gamma_multiply(0.85)`（透明度 85%）
- **边框色**：`window_stroke.color.gamma_multiply(0.3)`（透明度 30%）
- **阴影色**：`BLACK.gamma_multiply(0.12)`（透明度 12%）
- **悬停背景**：`selection.bg_fill.gamma_multiply(0.12-0.15)`
- **弱文本**：`weak_text_color()`（用于次要信息）

### 悬浮状态栏（Status Bar）
**位置**：底部居中，距离底部 12px  
**尺寸**：自适应宽度（最小 200px，最大 800px），高度 28px  
**样式**：
- 背景：`panel_fill × 0.7` 透明度
- 边框：1px，`window_stroke.color × 0.3` 透明度
- 圆角：10px
- 阴影：`offset: [0, 2], blur: 12, spread: 0, color: BLACK × 0.15`
- 内边距：左右 14px，上下 6px

**内容布局**：
- 文件名（加粗，11.5px）
- 图片尺寸（等宽字体，10.5px，弱文本色）
- 图片索引（10.5px）
- 缩放模式（Custom 模式使用橙色徽章：背景 `RGB(255,165,0) × 0.2`，文字 `RGB(255,140,0)`）
- 旋转角度（可选显示，10.5px）
- 文件大小（等宽字体，10.5px，弱文本色）

**分隔符**：自定义间距 8px + separator + 8px

### 菜单栏（Menu Bar）
**位置**：顶部固定  
**高度**：24px  
**背景**：`panel_fill`（跟随主题）  
**结构**：四个主菜单 - 📁 文件 | 🔍 查看 | ⚙️ 设置 | ❓ 帮助

**下拉菜单样式**：
- 背景：`panel_fill × 0.85` 透明度
- 边框：1px，`window_stroke.color × 0.3` 透明度
- 圆角：8px
- 阴影：`offset: [0, 4], blur: 16, spread: 0, color: BLACK × 0.12`
- 内边距：上下 6px，左右 12px
- 最小宽度：200px

**菜单项样式**：
- 高度：28px（含内边距）
- 字体：12px
- 图标：14px Emoji
- 图标与文字间距：8px
- 快捷键提示：Monospace 字体，10px，`weak_text_color()`，右对齐
- 悬停背景：`selection.bg_fill × 0.15` 透明度，圆角 4px
- 分隔线：1px，`window_stroke.color × 0.2` 透明度，上下各留 4px

### 弹窗/对话框（Dialogs）
**通用样式**：
- 锚点：`Align2::CENTER_CENTER`（居中显示）
- 不可折叠：`collapsible(false)`
- 不可调整大小：`resizable(false)`
- 固定尺寸：根据内容设定（如快捷键窗口 380×420px）

**快捷键窗口**：
- 尺寸：380×420px
- 分组标题：大写，10px，`weak_text_color()`
- 快捷键徽章：
  - 背景：`selection.bg_fill × 0.15`
  - 边框：`selection.bg_fill × 0.3`
  - 圆角：6px
  - 内边距：左右 8px，上下 3px
  - 字体：Monospace，11px，加粗
- 描述文字：12px，`weak_text_color()`

**设置窗口**：
- 尺寸：320×220px
- 分组标题：使用 `ui.heading()`
- 滑块：带文本标签
- 单选按钮：水平排列

**关于窗口**：
- 尺寸：320×200px
- 垂直居中对齐
- 应用名称：`ui.heading()`
- 版本号：普通文本
- GitHub 链接：`ui.hyperlink_to()`

### 缩略图导航（Thumbnail Navigation）
**显示条件**：图片尺寸大于可视区域时  
**位置**：右下角，距离边缘 10px  
**尺寸**：根据图片宽高比动态计算（最大 160×160px）  
**红框指示器**：
- 描边：2px，红色（`Color32::RED`）
- 描边类型：`StrokeKind::Inside`

### 欢迎界面（Welcome Screen）
**位置**：屏幕中心  
**内容**："点击打开\n或拖拽图片"  
**交互**：点击触发文件对话框

### 字体规范
- **正文字体**：默认字体族（Windows: 微软雅黑，其他：Noto Sans CJK）
- **等宽字体**：`FontFamily::Monospace`（用于尺寸、文件大小等技术信息）
- **字号层级**：
  - 标题：14-16px（heading）
  - 正文：12px
  - 次要信息：10-10.5px
  - 快捷键：11px（Monospace）

### 间距规范
- **小组件间距**：4px
- **中等间距**：8px
- **大间距**：12-14px
- **分组间距**：20px（弹窗内）
- **分隔线上下**：各 4px

### 圆角规范
- **小圆角**：4px（按钮、徽章、菜单项悬停）
- **中圆角**：6px（快捷键徽章）
- **大圆角**：8-10px（下拉菜单、状态栏、弹窗）

### 全屏模式
- 隐藏菜单栏和状态栏
- CentralPanel 填充整个窗口
- Esc 键退出全屏

### 国际化
- 所有用户可见文本必须通过 `TextKey` 枚举管理
- 禁止硬编码字符串
- 新增功能时同步添加中英文翻译
- 测试中英文切换后的布局适配

## 注意事项
- Windows 自动加载微软雅黑，其他系统尝试 Noto Sans CJK
- 配置位置：`%APPDATA%\fastview\settings.json` (Windows)
- 默认缓存 10 张图片，可在设置中调整
