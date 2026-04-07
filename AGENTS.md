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

## 注意事项
- Windows 自动加载微软雅黑，其他系统尝试 Noto Sans CJK
- 配置位置：`%APPDATA%\fastview\settings.json` (Windows)
- 默认缓存 10 张图片，可在设置中调整
