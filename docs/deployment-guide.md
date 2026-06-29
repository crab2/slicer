# 部署指南 — SLICER

## 概述

SLICER 使用 **GitHub Actions + Tauri Action** 自动构建和发布跨平台桌面应用。

---

## CI/CD 流水线

**触发条件：**

| 事件 | 说明 |
|------|------|
| `git push --tags "v*"` | 推送版本标签自动触发 |
| `workflow_dispatch` | 手动触发（可指定自定义标签） |

**构建矩阵：**

| 平台 | 目标架构 |
|------|---------|
| `macos-latest` | `aarch64-apple-darwin` (Apple Silicon) |
| `macos-latest` | `x86_64-apple-darwin` (Intel Mac) |
| `ubuntu-22.04` | `x86_64-unknown-linux-gnu` |
| `windows-latest` | `x86_64-pc-windows-msvc` |

**流水线步骤：**

1. **Checkout** — 拉取代码
2. **安装系统依赖** — Linux 安装 WebKit2GTK 等
3. **Setup Node.js** — LTS 版本，npm 缓存
4. **Install Rust** — dtolnay/rust-toolchain，macOS 附加跨架构 target
5. **Cache Rust** — swatinem/rust-cache 加速后续构建
6. **npm ci** — 安装前端依赖
7. **Build & Publish** — tauri-apps/tauri-action@v0 自动构建 + 创建 GitHub Release

---

## 版本发布流程

```bash
# 1. 确保所有更改已提交
git status

# 2. 更新版本号（如需）
# 编辑 src-tauri/tauri.conf.json 中的 version
# 编辑 package.json 中的 version

# 3. 创建版本标签并推送
git tag v0.1.0
git push origin v0.1.0

# 4. GitHub Actions 自动触发构建
# 等待构建完成后，Release 会自动出现在 GitHub Releases 页面
```

## 手动发布

也可以通过 GitHub Actions UI 手动触发：
1. 进入 GitHub 仓库 → Actions → Release workflow
2. 点击 "Run workflow"
3. 可选输入自定义 tag（留空则使用 Tauri 配置中的 `v__VERSION__`）

## 构建产物

每个平台的 Release 会包含：
- **Windows**: `.msi` 安装包 + `.exe` 安装程序
- **macOS**: `.dmg` 磁盘映像（Apple Silicon + Intel）
- **Linux**: `.deb` + `.AppImage`

## 应用签名

- **Windows**: 需配置代码签名证书（当前未配置）
- **macOS**: 需 Apple Developer 账号进行公证（当前未配置）
- **Linux**: 无需签名

---

## 系统要求（用户端）

| 平台 | 最低版本 |
|------|---------|
| Windows | 10 版本 1809+ |
| macOS | 11 (Big Sur)+ |
| Linux | Ubuntu 22.04+ / 等效发行版 |
