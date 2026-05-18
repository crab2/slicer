# Story 1.1: 初始化 Tauri React TypeScript Starter 与主导航

Status: done

## Story

As a 本地文档处理用户,
I want 基于官方 Tauri React TypeScript starter 初始化 slicer 桌面应用，并在启动后看到稳定可用的工作台首屏与主导航,
so that 我可以从安全的项目基础进入工作台、搜索和设置，并为后续导入、分析、索引与 API 能力建立一致的桌面交互基础。

## Acceptance Criteria

1. Given 当前仓库已包含 `_bmad`、`_bmad-output`、`.agents`、`.claude`、`docs` 等规划和上下文工件, when 开发者初始化官方 Tauri React TypeScript starter, then 初始化不得直接覆盖仓库根目录中的既有规划工件, and 应先在临时或受控目录 scaffold，再合并 `package.json`、前端 `src/` 和 `src-tauri/` 结构，并保留既有 BMad/文档目录。
2. Given starter 代码已合并到项目, when 开发者检查初始工程配置, then 项目应包含可运行的 Tauri v2、React、TypeScript、Vite 基础结构和必要依赖脚本, and starter 只作为应用壳基础，不得决定转换、分析、索引、存储、任务编排或 localhost API 的业务架构。
3. Given 用户在 Windows 本机启动 slicer 桌面应用, when 应用完成初始化并显示主窗口, then 首屏默认进入“工作台”视图，页面包含应用名称、当前工作区状态区域、主要操作入口占位，以及可识别的空状态, and 应用窗口在未配置工作区时不得崩溃，必须显示“尚未选择工作区”的明确状态。
4. Given 用户位于应用任意主视图, when 用户点击主导航中的“工作台”“搜索”“设置”入口, then 应用应在不重启窗口的情况下切换到对应视图, and 当前激活视图在导航中有明确的选中状态。
5. Given 前端需要调用 Rust/Tauri 后端能力, when 后续功能通过前端客户端发起命令调用, then 项目中应存在统一的 `tauriClient` 或等价客户端封装，用于集中处理 Tauri command 调用, and 业务组件不得直接散落调用底层 Tauri invoke API。
6. Given 开发者查看前端项目结构, when 检查应用壳、视图切换、共享组件和 Tauri 客户端封装, then 相关代码应按清晰模块组织，能够支撑后续工作台、搜索、设置页面继续扩展, and Story 1.1 不应实现真实导入、搜索、模型分析、索引构建或 localhost API 行为，只保留必要入口与占位状态。
7. Given 应用处于无工作区、加载中或基础错误状态, when 主界面渲染这些状态, then 用户应看到可理解的中文状态文案, and 状态展示应为后续统一错误模型与 Job 状态接入预留清晰位置。

## Tasks / Subtasks

- [x] 执行脚手架前置检查与安全初始化方案 (AC: 1, 2)
  - [x] 记录当前仓库根目录已有工件，确认 `_bmad`、`_bmad-output`、`.agents`、`.claude`、`docs` 不会被删除、移动或覆盖。
  - [x] 验证本机 Node.js、package manager、Rust toolchain 和 Tauri prerequisites；Vite 当前要求 Node.js `20.19+` 或 `22.12+`。
  - [x] 当前环境信号：`cargo 1.93.1` 与 `rustc 1.93.1` 可用；本次 create-story 运行时 `node --version` 返回“拒绝访问”，`npm` 未在 PATH 中找到。实现前必须重新确认可用 Node/npm/pnpm/yarn，或按官方 Tauri 文档使用可用的 `cargo create-tauri-app` 路径生成同等 `react-ts` starter。
  - [x] 使用官方 `create-tauri-app` 生成 React + TypeScript + Tauri v2 starter 到临时或受控目录，例如 `C:\tmp\slicer-tauri-scaffold` 或仓库外临时目录；不得在仓库根目录直接执行会覆盖现有文件的初始化。
  - [x] 从 scaffold 中合并必要的 root frontend 配置、`src/`、`src-tauri/`、Tauri capability/config 文件和 lockfile；合并前后检查 BMad/文档目录仍存在。

- [x] 建立最小可运行 Tauri React TypeScript 项目结构 (AC: 2, 5, 6)
  - [x] 保留官方 starter 的 Tauri v2、React、TypeScript、Vite 基础结构和脚本；不要手写一套非官方 starter 替代。
  - [x] 将前端结构整理为 `src/app`、`src/features/workbench`、`src/features/search`、`src/features/settings`、`src/components/common`、`src/lib`、`src/types`、`src/styles`。
  - [x] 创建 `src/lib/tauriClient.ts`，集中封装后续 Tauri `invoke` 调用入口。Story 1.1 只需要提供类型化占位/薄封装，不需要实现真实业务 command。
  - [x] Rust `src-tauri/` 只保留 starter 必需的最小 Tauri shell 能力；不要提前实现 SQLite、workspace service、job orchestrator、conversion provider、analysis provider、search provider 或 HTTP API。

- [x] 实现应用壳、主导航与三大占位视图 (AC: 3, 4, 6, 7)
  - [x] 默认首屏显示“工作台”，不是介绍页、营销页或 hero。
  - [x] 主导航包含“工作台”“搜索”“设置”三个入口，支持窗口内切换并保持当前选中状态。
  - [x] 工作台视图展示应用名称、当前工作区状态区域、“尚未选择工作区”空状态，以及后续选择工作区、导入、任务列表的占位区域。
  - [x] 搜索视图保留搜索输入、结果列表、图片预览、JSON 查看和索引状态的布局占位，但不得实现真实搜索。
  - [x] 设置视图保留工作目录、LibreOffice、模型、并发、localhost API 与隐私提示的布局占位，但不得实现真实持久化。
  - [x] 所有用户可见状态文案使用中文，尤其是未选择工作区、功能待接入、加载中和基础错误状态。

- [x] 落地克制的桌面工具视觉基础 (AC: 3, 4, 7)
  - [x] 使用简洁、克制、适合批量任务处理的信息密度；避免营销式大 hero、过度装饰渐变、纯展示型插画和过大的卡片式 landing。
  - [x] 导航、按钮、空状态、面板、状态徽标等 UI 必须稳定、不互相遮挡，常见桌面尺寸下文本不溢出。
  - [x] 可以建立 `components/common` 的少量基础组件，例如 `Button`、`Tabs`、`EmptyState`、`ErrorMessage`，但不要为了 Story 1.1 过度抽象。

- [x] 验证 scaffold、构建和桌面启动路径 (AC: 2, 3, 4)
  - [x] 运行 package manager install，确保 lockfile 与实际 package manager 一致。
  - [x] 运行 TypeScript/front-end build，例如 `npm run build` 或等价命令。
  - [x] 运行 Rust/Tauri 检查或构建，例如 `cargo check`、`npm run tauri dev` 或等价命令；如果无法在当前环境打开窗口，至少验证 frontend build 和 Rust compile，并记录阻塞原因。
  - [x] 人工或浏览器/截图验证主窗口首屏为工作台，导航可切换到搜索和设置，未选择工作区状态清楚可见。
  - [x] 确认 `_bmad`、`_bmad-output`、`.agents`、`.claude`、`docs` 在实现后仍位于仓库根目录且未被 starter 覆盖。

### Review Findings

- [x] [Review][Patch] HTML document language remains English while the app shell is Chinese [index.html:2]
- [x] [Review][Patch] Story File List omits generated Rust application lockfile [`_bmad-output/implementation-artifacts/1-1-初始化-tauri-react-typescript-starter-与主导航.md`:190]

## Dev Notes

### Scope Boundaries

- Story 1.1 是项目初始化与应用壳故事，只交付可运行的 Tauri React TypeScript starter、主导航、三大视图占位、统一 `tauriClient` 入口和基础 UI 状态。
- 不要实现真实导入、转换、模型分析、BM25 索引、SQLite schema、Job Orchestrator、workspace reconciliation、localhost HTTP API 或 token 机制。这些属于后续 Story 1.2+、Epic 2+、Epic 3+、Epic 4+、Epic 5。
- 也不要把这些后续能力写死到前端本地状态。此故事只能为后续能力留下清晰入口和文件边界。

### Architecture Compliance

- 官方 Tauri 文档说明 `create-tauri-app` 用于用官方维护模板创建新 Tauri 项目，支持 React 等前端模板；本项目架构已选择 official `create-tauri-app` + `react-ts` starter。[Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/architecture.md:195`]
- 初始化必须先 scaffold 到临时或受控目录，再合并 `package.json`、`src/`、`src-tauri/` 等结构，保留 `_bmad`、`_bmad-output`、`.agents`、`.claude`、`docs`。[Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/architecture.md:205`]
- Starter 是应用壳，不决定转换、分析、索引、存储、任务编排或 localhost API；这些必须在后续 Rust application services 和 domain modules 中实现。[Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/architecture.md:201`]
- 前端使用 React + TypeScript；持久化/工作区状态归 Rust + SQLite，React 只保留 selected tab、selected task/result、form draft、loading 等视图本地状态。[Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/architecture.md:383`]
- 前端业务组件不得散落直接调用 `invoke`；后续 Tauri command 调用必须集中在 `src/lib/tauriClient.ts` 或等价 typed client。[Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/architecture.md:998`]
- 后续 Tauri commands 只能做请求 DTO 校验、调用 services、返回 DTO/job ID；不能直接 spawn LibreOffice、写 SQLite、改索引文件或调用模型 API。Story 1.1 不需要实现这些 commands，但文件组织不能与该边界冲突。[Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/architecture.md:972`]

### Frontend Structure Guardrails

- 目标前端组织：
  - `src/main.tsx`
  - `src/App.tsx`
  - `src/app/AppShell.tsx`
  - `src/app/navigation.ts`
  - `src/features/workbench/WorkbenchPage.tsx`
  - `src/features/search/SearchPage.tsx`
  - `src/features/settings/SettingsPage.tsx`
  - `src/components/common/*`
  - `src/lib/tauriClient.ts`
  - `src/styles/globals.css`
- `features/workbench` 后续会承载导入、转换、分析、任务列表、重试、索引重建 UI；Story 1.1 只放占位布局。
- `features/search` 后续会承载查询、结果列表、预览、JSON viewer 和索引状态 UI；Story 1.1 只放不可执行占位。
- `features/settings` 后续会承载工作区、LibreOffice、模型 provider、API server 和隐私设置 UI；Story 1.1 只放字段/区域占位。
- `components/common` 只放真正复用的基础组件。不要把页面专属内容塞进 common。

### UX Requirements

- PRD 要求第一屏为工作台，不提供营销页或介绍页；主导航包含工作台、搜索、设置。[Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/prd.md:119`]
- 工作台最终需要当前工作目录、选择/更改目录、拖拽、文件选择、任务列表、转换/分析/重试/索引入口；本故事只显示这些能力的稳定占位和空状态。[Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/prd.md:125`]
- 搜索页最终需要输入、结果列表、图片预览、页面 JSON、无结果和索引状态；本故事只建立页面结构，不接真实 search service。[Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/prd.md:141`]
- 设置页最终需要工作目录、LibreOffice、模型、默认 DPI、并发、隐私提示等；本故事只建立页面结构，不保存设置。[Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/prd.md:153`]
- 视觉风格必须简洁、克制、桌面工具感强，不使用营销式大 hero 或过度装饰性渐变背景；错误状态明确、可定位、可恢复。[Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/prd.md:170`]

### Technical Version Notes

- 使用官方 `create-tauri-app@latest` 生成项目时，让生成器锁定当前兼容依赖；不要在 story 实现中硬编码过时版本号。
- 架构文档记录：Tauri docs 推荐 `create-tauri-app`；React docs 当时最新为 React `19.2`；Vite 要求 Node.js `20.19+` 或 `22.12+`。[Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/architecture.md:152`]
- 本次额外核验的官方信息：Tauri v2 Create Project 页面仍将 `create-tauri-app` 作为官方项目创建工具，并列出 `npm create tauri-app@latest`、`cargo create-tauri-app` 等路径；Vite guide 当前显示 Vite `8.0.10` 且要求 Node.js `20.19+` 或 `22.12+`；React versions/blog 页面显示 React `19.2` 已发布。
- 实现时若 package registry 或网络不可用，先记录环境阻塞，不要手写一个“看起来像 starter”的非官方结构来假装完成。

### Existing Project State

- 当前仓库根目录在 create-story 时只发现规划和 BMad 工件：`.agents`、`.claude`、`docs`、`_bmad`、`_bmad-output`。
- 当前 implementation artifacts 中只有 `sprint-status.yaml`；Story 1.1 是第一条实现故事，没有 previous story intelligence。
- 当前目录不是 git repository，create-story 无法读取最近提交历史；实现 agent 不应假设已有前端/Rust 代码模式。

### Testing Requirements

- 最低验证：
  - package install 成功并产生/更新正确 lockfile。
  - frontend build 成功。
  - Rust/Tauri compile/check 成功，或明确记录缺少 Node/npm/WebView2/系统依赖导致无法完成的步骤。
  - UI 首屏为工作台；主导航可切换“工作台”“搜索”“设置”；无工作区状态中文可见。
  - BMad/文档目录未被覆盖或移动。
- 后续架构测试位置约定：
  - Rust integration tests: `src-tauri/tests/`
  - Rust fixtures: `src-tauri/fixtures/`
  - Frontend component tests: 组件旁 `*.test.tsx`
  - Tauri E2E smoke tests: `tests/e2e/`
- Story 1.1 可先不建立完整测试套件，但如果 starter 自带 lint/build/test 命令，应保证它们通过或记录阻塞原因。

### Anti-Patterns to Avoid

- 不要在仓库根目录直接运行会覆盖当前文件的 scaffolding 命令。
- 不要删除、重命名或移动 `_bmad`、`_bmad-output`、`.agents`、`.claude`、`docs`。
- 不要创建营销首页、产品介绍页或 hero 作为首屏。
- 不要把真实业务逻辑放进 React 组件、Tauri command 或 `tauriClient` mock 中。
- 不要在 Story 1.1 中提前决定 SQLite schema、document/page status enums、`page_id` 算法、search provider、model provider 或 localhost API token 策略。
- 不要让前端组件手动拼接未来 workspace 内部路径。

### References

- [Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/epics.md:220`] Epic 1 目标、覆盖 FR 与实现备注。
- [Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/epics.md:228`] Story 1.1 原始用户故事与验收标准。
- [Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/architecture.md:135`] 技术域、Windows 优先、Rust + Tauri、SQLite、LibreOffice、BM25、localhost API。
- [Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/architecture.md:195`] 官方 Tauri React TypeScript starter 选择。
- [Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/architecture.md:383`] 前端状态管理和导航模式。
- [Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/architecture.md:740`] 目标项目目录结构。
- [Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/architecture.md:963`] 架构边界。
- [Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/architecture.md:1159`] 开发、构建和打包工作流。
- [Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/prd.md:119`] 信息架构和主导航。
- [Source: `D:/AIProject/slicer/_bmad-output/planning-artifacts/prd.md:170`] 视觉风格约束。
- Official reference: Tauri Create a Project, `https://v2.tauri.app/start/create-project/`
- Official reference: Vite Getting Started, `https://vite.dev/guide/`
- Official reference: React Versions, `https://react.dev/versions`

## Dev Agent Record

### Agent Model Used

GPT-5 Codex

### Debug Log References

- 2026-05-18T14:00:00+08:00 - Loaded BMad dev-story workflow fallback because `python3` is not available in this PowerShell environment.
- 2026-05-18T14:00:00+08:00 - Confirmed repository root still contains `.agents`, `.claude`, `docs`, `_bmad`, and `_bmad-output`; no starter scaffold was run in the repository root.
- 2026-05-18T14:00:00+08:00 - Toolchain check: bundled Codex Node at `C:\Users\51314\.cache\codex-runtimes\codex-primary-runtime\dependencies\node\bin\node.exe` reports `v24.14.0`, satisfying Vite's Node requirement; PATH `node.exe` fails with access denied; `npm`, `pnpm`, and `yarn` are not available on PATH.
- 2026-05-18T14:00:00+08:00 - Toolchain check: `cargo 1.93.1` and `rustc 1.93.1` are available; `cargo create-tauri-app` is not installed.
- 2026-05-18T14:00:00+08:00 - Attempted `cargo install create-tauri-app --locked --root C:\tmp\cargo-tools` with `CARGO_HOME=C:\tmp\cargo-home`; sandboxed network failed to reach `https://index.crates.io/config.json`.
- 2026-05-18T14:00:00+08:00 - Requested escalated network install for official `create-tauri-app`; approval system rejected the action, so official scaffold generation could not continue in this session.
- 2026-05-18T14:00:00+08:00 - Searched `C:\tmp` for cached `create-tauri-app`/Tauri React TypeScript scaffold/template artifacts; none were found.
- 2026-05-18T14:20:00+08:00 - User installed Node/npm and `create-tauri-app`; Codex sandbox still cannot execute nvm Node/npm paths, but `cargo create-tauri-app --version` reports `4.7.0`.
- 2026-05-18T14:20:00+08:00 - Ran official `cargo create-tauri-app slicer-tauri-scaffold --template react-ts --manager npm --tauri-version 2 --identifier com.slicer.app --yes --force` in `C:\tmp`; scaffold completed without writing to repository root.
- 2026-05-18T14:20:00+08:00 - Merged scaffold root config, `src/`, `src-tauri/`, and `public/` into the repository; confirmed `.agents`, `.claude`, `docs`, `_bmad`, and `_bmad-output` remain in the repository root.
- 2026-05-18T14:20:00+08:00 - Replaced starter demo UI with `AppShell`, navigation, workbench/search/settings placeholder views, shared basic components, global desktop-tool styling, and a typed `src/lib/tauriClient.ts` invoke wrapper.
- 2026-05-18T14:20:00+08:00 - Kept Rust side to the minimal official Tauri shell with opener plugin; removed starter `greet` command to avoid story-external business commands.
- 2026-05-18T14:20:00+08:00 - Validation blocked inside Codex sandbox: direct nvm Node/npm execution returns access denied, bundled Node lacks npm, and `cargo check` triggers a Windows sandbox setup refresh failure before project compilation.
- 2026-05-18T14:35:00+08:00 - Adjusted Workbench spacing between the empty workspace state and action buttons, replaced the sidebar letter mark with a custom SLICER logo mark, changed visible brand text to uppercase `SLICER`, and added an SVG favicon.
- 2026-05-18T14:45:00+08:00 - Verified dependency installation artifacts exist: `node_modules/` and `package-lock.json` are present.
- 2026-05-18T14:45:00+08:00 - Ran TypeScript check with local installed dependency via bundled Node: `node node_modules/typescript/bin/tsc`; passed with no output.
- 2026-05-18T14:45:00+08:00 - Ran Vite production build with local installed dependency via bundled Node: `node node_modules/vite/bin/vite.js build`; passed, producing `dist/index.html`, CSS, and JS assets.
- 2026-05-18T14:45:00+08:00 - User-provided terminal evidence shows `npm run tauri dev` launched Vite at `http://localhost:1420/`, compiled Rust/Tauri dev target, and ran `target\debug\slicer.exe`.
- 2026-05-18T14:45:00+08:00 - User-provided screenshots verify Workbench first screen, Search view, Settings view, active navigation state, uppercase `SLICER` branding, and visible "尚未选择工作区" no-workspace status.
- 2026-05-18T14:55:00+08:00 - Applied code review patches: changed `index.html` language to `zh-CN` and added `src-tauri/Cargo.lock` to the story File List.

### Completion Notes List

- create-story context engine analysis completed; story is ready for implementation.
- Official Tauri v2 React TypeScript scaffold was generated in `C:\tmp\slicer-tauri-scaffold` and safely merged into the repository without deleting or moving BMad/planning artifacts.
- Implemented a restrained desktop application shell with default Workbench first screen, in-window navigation for Workbench/Search/Settings, Chinese empty/loading/error status copy, and placeholders for future import/search/settings capabilities.
- Added `src/lib/tauriClient.ts` as the single typed Tauri invoke wrapper and kept `src-tauri` limited to minimal shell responsibilities.
- Refined the Workbench visual polish by adding a clear gap above the primary action row and replacing the starter-style letter tile with a custom SLICER logo treatment.
- Completed validation: npm install artifacts are present, TypeScript check passed, Vite production build passed, Tauri dev compile/run is evidenced by user terminal screenshot, and UI smoke verification is evidenced by user screenshots.
- Story 1.1 is complete and ready for code review.
- Code review patches were applied and all review findings are resolved.

### File List

- `_bmad-output/implementation-artifacts/sprint-status.yaml`
- `_bmad-output/implementation-artifacts/1-1-初始化-tauri-react-typescript-starter-与主导航.md`
- `.gitignore`
- `dist/assets/index-BQKJKGKT.css`
- `dist/assets/index-D53hCE4c.js`
- `dist/index.html`
- `dist/slicer-logo.svg`
- `dist/tauri.svg`
- `dist/vite.svg`
- `index.html`
- `package-lock.json`
- `package.json`
- `tsconfig.json`
- `tsconfig.node.json`
- `vite.config.ts`
- `public/tauri.svg`
- `public/slicer-logo.svg`
- `public/vite.svg`
- `src/App.tsx`
- `src/app/AppShell.tsx`
- `src/app/navigation.ts`
- `src/assets/react.svg`
- `src/components/common/Button.tsx`
- `src/components/common/EmptyState.tsx`
- `src/components/common/ErrorMessage.tsx`
- `src/components/common/StatusBadge.tsx`
- `src/features/search/SearchPage.tsx`
- `src/features/settings/SettingsPage.tsx`
- `src/features/workbench/WorkbenchPage.tsx`
- `src/lib/tauriClient.ts`
- `src/main.tsx`
- `src/styles/globals.css`
- `src/types/app.ts`
- `src/vite-env.d.ts`
- `src-tauri/.gitignore`
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/build.rs`
- `src-tauri/capabilities/default.json`
- `src-tauri/icons/128x128.png`
- `src-tauri/icons/128x128@2x.png`
- `src-tauri/icons/32x32.png`
- `src-tauri/icons/Square107x107Logo.png`
- `src-tauri/icons/Square142x142Logo.png`
- `src-tauri/icons/Square150x150Logo.png`
- `src-tauri/icons/Square284x284Logo.png`
- `src-tauri/icons/Square30x30Logo.png`
- `src-tauri/icons/Square310x310Logo.png`
- `src-tauri/icons/Square44x44Logo.png`
- `src-tauri/icons/Square71x71Logo.png`
- `src-tauri/icons/Square89x89Logo.png`
- `src-tauri/icons/StoreLogo.png`
- `src-tauri/icons/icon.icns`
- `src-tauri/icons/icon.ico`
- `src-tauri/icons/icon.png`
- `src-tauri/src/lib.rs`
- `src-tauri/src/main.rs`
- `src-tauri/tauri.conf.json`
