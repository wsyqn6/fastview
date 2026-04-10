# 🖼️ FastView Agent 配置

## 🎯 定位
专注图片查看，⚡ 启动 <1s，📦 体积 ~5MB。

## 🛠️ 技术栈
| 组件 | 选型 |
|------|------|
| 🎨 GUI | eframe/egui 0.32 |
| 🖼️ 图片 | image 0.25 |
| 📂 对话框 | rfd 0.16 |
| ⚙️ 配置 | serde + dirs |

## 📁 项目结构
```
src/
├── 🚀 main.rs      # 入口
├── 🧠 app.rs       # 主逻辑
├── 📐 types.rs     # 类型
├── 🔤 fonts.rs     # 字体
└── 🌐 i18n.rs      # 文案
```

## ⌨️ 快捷键
- `←/→` 上一张/下一张 | `+/-` 缩放 | `0/1/2` 适应/原始/填充
- `R`/`Shift+R` 旋转 | `F` 全屏 | `V` 无边框 | `Space` 按住拖动 | `H` 帮助

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

