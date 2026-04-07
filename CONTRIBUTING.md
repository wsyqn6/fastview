# Contributing to FastView

> **Current Version**: 0.1.1 (Development Preview)

Thank you for your interest in contributing to FastView! This document provides guidelines and instructions for contributing.

## 🚀 Getting Started

### Prerequisites

- Rust 1.70 or later ([installation guide](https://www.rust-lang.org/tools/install))
- Git
- Your favorite code editor (VS Code with rust-analyzer recommended)

### Setup

1. Fork the repository
2. Clone your fork:
   ```bash
   git clone https://github.com/yourusername/fastview.git
   cd fastview
   ```
3. Create a branch for your changes:
   ```bash
   git checkout -b feature/your-feature-name
   ```

## 📝 Development Workflow

### Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run in debug mode
cargo run

# Run in release mode
cargo run --release
```

### Code Style

FastView follows standard Rust conventions:

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt --check

# Lint code
cargo clippy

# Run tests
cargo test
```

### Commit Messages

We follow [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` - New features
- `fix:` - Bug fixes
- `docs:` - Documentation changes
- `style:` - Code style changes (formatting, etc.)
- `refactor:` - Code refactoring
- `test:` - Adding or updating tests
- `chore:` - Maintenance tasks

Examples:
```
feat: add drag and drop support for images
fix: correct EXIF orientation handling
docs: update README with installation instructions
```

## 🎯 What We're Looking For

### Good First Issues

Look for issues labeled `good first issue` - these are great starting points for new contributors.

### Priority Areas

Based on our roadmap:
- Performance optimizations
- Bug fixes
- Documentation improvements
- UI/UX enhancements
- Additional image format support

### What We Don't Accept

To maintain FastView's core philosophy of being "fast, small, and simple":

- ❌ Image editing features (crop, filters, adjustments)
- ❌ File management features (delete, move, rename)
- ❌ Cloud sync or sharing features
- ❌ Plugin systems
- ❌ Complex theme customization

## 🐛 Reporting Bugs

Before creating a bug report:

1. Check existing issues to avoid duplicates
2. Test with the latest version
3. Gather information:
   - Operating system and version
   - FastView version
   - Steps to reproduce
   - Expected vs actual behavior
   - Screenshots if applicable

### Bug Report Template

```markdown
**Describe the bug**
A clear description of what the bug is.

**To Reproduce**
Steps to reproduce the behavior:
1. Open image '...'
2. Press key '....'
3. See error

**Expected behavior**
What you expected to happen.

**Screenshots**
If applicable, add screenshots.

**Environment:**
 - OS: [e.g. Windows 11, Ubuntu 22.04]
 - Version: [e.g. 1.0.0]
```

## 💡 Feature Requests

We welcome feature suggestions! Please:

1. Check if the feature aligns with FastView's philosophy
2. Search existing issues to avoid duplicates
3. Clearly describe the use case
4. Consider implementation complexity

## 🔍 Code Review Process

1. Ensure your code passes `cargo fmt`, `cargo clippy`, and `cargo test`
2. Update documentation if needed
3. Add tests for new functionality
4. Keep pull requests focused and manageable
5. Reference related issues in your PR description

## 📚 Documentation

When adding features or making changes:

- Update README.md if user-facing changes
- Add inline comments for complex logic
- Update AGENTS.md if workflow changes
- Keep documentation clear and concise

## 🤝 Community Guidelines

- Be respectful and inclusive
- Help others learn and grow
- Focus on constructive feedback
- Follow the project's code of conduct

## ❓ Questions?

Feel free to open an issue for questions not covered here.

---

Thank you for contributing to FastView! 🎉
