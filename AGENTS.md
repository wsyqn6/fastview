# 🖼️ FastView Agent 配置

## 🎯 目标
专注图片查看，⚡ 启动 <1s，📦 体积 ~5MB。

## 🛠️ 技术栈
| 组件 | 选型 |
|------|------|
| 🎨 GUI | eframe/egui 0.34 |
| 🖼️ 图片 | image 0.25 |
| 📂 对话框 | rfd 0.16 |
| ⚙️ 配置 | serde + dirs |

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
- 缓存策略：LRU + 纹理池复用
- 异步加载，不阻塞 UI 线程

### 安全规范
- 路径操作使用 `PathBuf`，验证文件存在性
- 网络请求设置合理超时
- 锁使用 `lock_or_recover()` 避免中毒

## 🚧 功能边界
✅ 查看、缩放、旋转、缓存、中英切换  
❌ 编辑、管理、云同步

## 🎨 UI 要点
- 毛玻璃效果、4/8/12 间距、整数字号
- 状态栏居中悬浮，全屏隐藏菜单
- 文案通过 `TextKey` 枚举管理

## 💾 配置路径
`%APPDATA%\fastview\settings.json`

## 📉 体积优化
`lto=true`, `opt-level="z"`, `strip=true`, `panic="abort"`

## 📦 构建命令
```bash
# 开发构建
cargo build

# Release 构建（最小体积）
cargo build --release

# 运行
cargo run --release -- <图片路径>
```