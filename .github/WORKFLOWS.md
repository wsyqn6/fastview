# GitHub Actions 使用指南

## 📋 概述

本项目配置了两个 GitHub Actions workflows：

1. **CI (ci.yml)** - 持续集成，每次推送和 PR 时自动运行
2. **Release (release.yml)** - 自动发布，打 tag 时触发

---

## 🔄 CI Workflow

### 触发条件
- 推送到 `main` 或 `develop` 分支
- 创建 Pull Request 到 `main` 分支

### 执行任务

#### 1. 代码质量检查 (Check)
```bash
✅ cargo fmt --check      # 代码格式检查
✅ cargo clippy           # 代码 linting
✅ cargo check            # 编译检查
```

#### 2. 测试 (Test)
```bash
✅ cargo test             # 运行所有测试
```
在三个平台上运行：
- Ubuntu Linux
- Windows
- macOS

#### 3. 构建 (Build)
为三个平台分别构建 release 版本：
- **Linux**: `target/release/fastview`
- **Windows**: `target/release/fastview.exe`
- **macOS**: `target/release/fastview`

### 查看结果

1. 进入项目的 **Actions** 标签页
2. 点击具体的 workflow run
3. 查看每个 job 的详细日志
4. 下载构建产物（Artifacts）

---

## 🚀 Release Workflow

### 触发条件
推送以 `v` 开头的 tag，例如：
```bash
git tag v1.0.0
git push origin v1.0.0
```

### 执行流程

#### 1. 多平台构建
- ✅ Linux x86_64
- ✅ Windows x86_64
- ✅ macOS x86_64

#### 2. 打包
- **Linux**: `fastview-linux-x86_64.tar.gz`
- **Windows**: `fastview-windows-x86_64.zip`
- **macOS**: `fastview-macos-x86_64.tar.gz`

#### 3. 创建 GitHub Release
自动创建 Release 并上传所有平台的二进制文件

#### 4. 生成发布说明
从 `CHANGELOG.md` 自动提取当前版本的变更内容

---

## 📝 发布新版本步骤

### 1. 更新 CHANGELOG.md

```markdown
## [1.1.0] - 2026-04-08

### Added
- New feature description

### Fixed
- Bug fix description
```

### 2. 更新版本号

编辑 `Cargo.toml`:
```toml
[package]
version = "1.1.0"  # 更新版本号
```

### 3. 提交更改

```bash
git add .
git commit -m "chore: release v1.1.0"
git push
```

### 4. 创建并推送 Tag

```bash
# 创建 tag
git tag v1.1.0

# 推送 tag（触发 Release workflow）
git push origin v1.1.0
```

### 5. 等待自动化完成

1. 进入 GitHub → Actions
2. 查看 Release workflow 运行状态
3. 完成后，新的 Release 会自动创建
4. 下载链接会自动出现在 Releases 页面

---

## 🔧 自定义配置

### 修改触发分支

编辑 `.github/workflows/ci.yml`:
```yaml
on:
  push:
    branches: [ main, develop, feature/* ]  # 添加更多分支
```

### 添加更多平台

在 matrix 中添加新平台：
```yaml
strategy:
  matrix:
    os: [ubuntu-latest, windows-latest, macos-latest, macos-14]
```

### 启用缓存优化

已配置 `Swatinem/rust-cache@v2`，自动缓存：
- Cargo registry
- Build artifacts
- Target directory

---

## 🐛 故障排查

### Workflow 失败

1. 查看 Actions 日志
2. 定位失败的 step
3. 本地复现问题
4. 修复后重新推送

### 常见错误

#### 1. Linux 依赖缺失
```bash
# 错误：libgtk-3-dev not found
# 解决：确保 workflow 中包含依赖安装步骤
sudo apt-get install -y libgtk-3-dev
```

#### 2. 权限不足
```bash
# 错误：permission denied
# 解决：检查 GITHUB_TOKEN 权限配置
permissions:
  contents: write
```

#### 3. Tag 格式错误
```bash
# 错误：tag 不以 v 开头
# 正确：v1.0.0
# 错误：1.0.0
```

---

## 📊 监控和优化

### 构建时间优化

当前配置已包含：
- ✅ Rust 依赖缓存
- ✅ 增量编译
- ✅ 并行构建

### 减少构建时间

1. **跳过不必要的测试**
```yaml
# 仅在某些文件变化时运行
paths-ignore:
  - '**.md'
  - 'docs/**'
```

2. **使用更快的 runner**
```yaml
runs-on: ubuntu-latest  # 标准
runs-on: ubuntu-24.04   # 更新版本，可能更快
```

---

## 🔗 相关资源

- [GitHub Actions 文档](https://docs.github.com/en/actions)
- [Rust Actions 市场](https://github.com/marketplace?type=actions&query=rust)
- [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain)
- [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache)
- [softprops/action-gh-release](https://github.com/softprops/action-gh-release)

---

## 💡 最佳实践

1. **保持 workflow 简洁** - 只包含必要的步骤
2. **使用缓存** - 加速重复构建
3. **并行执行** - 利用 matrix strategy
4. **清晰的命名** - 便于识别不同的 jobs
5. **定期更新 actions** - 使用最新版本
6. **测试 workflow** - 在 dry-run 模式验证

---

*Last updated: 2026-04-07*
