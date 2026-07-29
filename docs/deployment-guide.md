# SLICER 部署手册

> 面向发布负责人、测试人员和现场交付人员。最后更新：2026-07-02。

## 1. 部署形态

SLICER 以 Tauri 2 桌面应用发布。应用本身不需要部署中心化服务，用户数据默认保存在用户选择的本地工作区。

发布内容主要包括：

- 桌面安装包或便携产物。
- 前端静态资源，打包进 Tauri 应用。
- Rust 后端二进制，内嵌 Tauri commands 和可选 localhost API。
- 不内置用户工作区、不内置模型 API key、不内置 LibreOffice。

## 2. 构建环境要求

### 2.1 通用要求

| 工具 | 建议版本 | 用途 |
| --- | --- | --- |
| Node.js | `20.19+` 或 `22.12+` | 安装依赖、Vite/TypeScript 构建 |
| npm | 随 Node.js LTS | 前端依赖管理 |
| Rust | stable | Rust/Tauri 后端编译 |
| Cargo | 随 Rust stable | Rust 依赖与测试 |
| Tauri CLI | `@tauri-apps/cli` v2 | 桌面应用打包 |
| Git | 2.x+ | 版本与标签管理 |

### 2.2 Windows 构建要求

- Microsoft WebView2 Runtime。
- Visual Studio C++ Build Tools 或包含 MSVC 工具链的 Visual Studio。
- Windows SDK。

### 2.3 macOS 构建要求

- Xcode Command Line Tools。
- 如需正式分发，需要 Apple Developer 证书、公证配置和签名环境变量。

### 2.4 Linux 构建要求

Ubuntu 22.04 类环境需要：

```bash
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf
```

## 3. 本地构建流程

### 3.1 安装依赖

```bash
npm install
```

CI 中使用：

```bash
npm ci
```

### 3.2 验证前端

```bash
npm run build
```

该命令会先执行 TypeScript 编译，再执行 Vite 生产构建。

### 3.3 验证 Rust 后端

```bash
cd src-tauri
cargo check
cargo test
```

### 3.4 打包桌面应用

```bash
npm run tauri build
```

常见输出目录：

```text
src-tauri/target/release/
src-tauri/target/release/bundle/
```

## 4. 版本号管理

发布前需要同步版本号：

| 文件 | 字段 |
| --- | --- |
| `package.json` | `version` |
| `src-tauri/tauri.conf.json` | `version` |
| `src-tauri/Cargo.toml` | `package.version` |

当前仓库版本为 `0.1.0`。

建议发布分支流程：

```bash
git status
npm run build
cd src-tauri && cargo test && cd ..
git tag v0.1.0
git push origin v0.1.0
```

## 5. GitHub Actions 发布

发布流水线位于 `.github/workflows/release.yml`。

### 5.1 触发方式

| 触发 | 说明 |
| --- | --- |
| `push` tag `v*` | 推送 `v0.1.0` 这类标签后自动发布 |
| `workflow_dispatch` | 在 GitHub Actions 页面手动触发，可输入 tag |

### 5.2 构建矩阵

| Runner | 目标 |
| --- | --- |
| `windows-latest` | Windows x86_64 |
| `macos-latest` + `--target aarch64-apple-darwin` | macOS Apple Silicon |
| `macos-latest` + `--target x86_64-apple-darwin` | macOS Intel |
| `ubuntu-22.04` | Linux x86_64 |

### 5.3 CI 主要步骤

1. `actions/checkout@v4` 拉取代码。
2. Linux 安装 WebKit2GTK、OpenSSL、AppIndicator、rsvg、patchelf 等依赖。
3. `actions/setup-node@v4` 安装 Node.js LTS 并启用 npm 缓存。
4. `dtolnay/rust-toolchain@stable` 安装 Rust stable，macOS 增加双架构 target。
5. `swatinem/rust-cache@v2` 缓存 `src-tauri -> target`。
6. `npm ci` 安装前端依赖。
7. `tauri-apps/tauri-action@v0` 构建并创建 GitHub Release。

### 5.4 Release 命名

流水线中：

- `tagName`：tag push 时使用 `github.ref_name`；手动触发时使用输入 tag；留空则使用 `v__VERSION__`。
- `releaseName`：`SLICER <tag>`。
- `releaseDraft`：`false`。
- `prerelease`：`false`。

## 6. 产物说明

不同平台产物由 Tauri bundle 自动决定，通常包括：

| 平台 | 典型产物 |
| --- | --- |
| Windows | `.msi`、`.exe` 安装器 |
| macOS | `.dmg`、`.app` bundle |
| Linux | `.deb`、`.AppImage` |

Windows 是当前第一优先验证平台。正式对外发布前，应至少完成 Windows 安装、启动、导入、分析、索引、搜索与 API smoke test。

## 7. 发布前检查清单

### 7.1 代码与配置

- `git status` 干净或只包含预期变更。
- `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 版本一致。
- `src-tauri/tauri.conf.json` 中 `productName`、`identifier`、窗口尺寸和 icon 配置符合预期。
- `.github/workflows/release.yml` 仍使用正确 runner 和 Tauri action。
- 不含真实 API key、Bearer token、用户工作区路径或个人文件。

### 7.2 自动验证

```bash
npm run build
npm run test:media-boundaries
cd src-tauri
cargo check
cargo test
```

### 7.3 手工 smoke test

1. 启动应用。
2. 选择新的临时工作区。
3. 导入 PDF 或图片。
4. 如需验证 Office，配置 LibreOffice 并导入 DOCX/PPTX。
5. 配置模型 profile 和 API key。
6. 确认隐私提示后分析一页。
7. 构建 BM25 索引。
8. 搜索关键词并打开结果 JSON。
9. 启用 Localhost API。
10. 调用 `/health`、`/search`，并用 Bearer token 调用 `/indexes/rebuild`。

## 8. 签名与安全

当前仓库未配置生产签名。

| 平台 | 当前状态 | 正式发布建议 |
| --- | --- | --- |
| Windows | 未配置代码签名 | 配置企业代码签名证书，降低安装警告 |
| macOS | 未配置签名/公证 | 配置 Apple Developer ID、notarization、staple |
| Linux | 通常无需签名 | 可按发行渠道补充包签名 |

安全注意：

- 不要把 API key 写入构建产物、日志、Release note 或截图。
- Localhost API 默认只允许 `127.0.0.1`，不要改为公网监听。
- `POST /indexes/rebuild` 必须保持 Bearer token 保护。
- 发布包不应包含用户工作区数据。

## 9. 用户端系统要求

| 平台 | 最低/建议 |
| --- | --- |
| Windows | Windows 10 1809+，建议安装 WebView2 Runtime |
| macOS | macOS 11 Big Sur+ |
| Linux | Ubuntu 22.04+ 或同等支持 WebKitGTK 的发行版 |

运行期可选依赖：

- LibreOffice：仅 Office 文档导入需要。
- 模型 API 网络访问：仅页面分析需要。

## 10. 回滚策略

桌面应用回滚通常由用户安装旧版本完成。

发布侧建议：

1. 保留旧版本 GitHub Release。
2. 不覆盖已有 tag。
3. 如果新 Release 有严重问题，先标记 Release 说明，再发布修复版本。
4. 数据 schema 迁移只向前执行；如需回滚应用，请先备份工作区。

用户数据侧建议：

```powershell
Copy-Item -Recurse -LiteralPath "<workspace>" -Destination "<workspace>-backup-before-upgrade"
```

最重要的可恢复文件是：

```text
<workspace>/app.db
<workspace>/originals/
<workspace>/pages/
```

`metadata/pages.jsonl` 和 `indexes/bm25/` 可从 SQLite 与分析结果重新生成。

## 11. 发布记录模板

```markdown
## SLICER vX.Y.Z

发布日期：
发布负责人：
代码提交：

### 新增
- 

### 修复
- 

### 已知问题
- 

### 验证
- npm run build：
- npm run test:media-boundaries：
- cargo check：
- cargo test：
- Windows smoke test：
- Localhost API smoke test：
```
