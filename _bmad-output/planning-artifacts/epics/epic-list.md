# Epic List

### Epic 1: 工作区、导航与工作台分流体验

用户打开 slicer 后，可以直接进入清晰的工作台概览，看到工作区状态、媒体/页面/失败/索引摘要，并通过左侧 sidebar 进入 `媒体导入`、`媒体管理`、`模型分析`、`BM25 索引`、`搜索`、`设置` 等对应功能区。工作台只做展示和跳转，不承载具体重操作。

**FRs covered:** FR1, FR13, FR14, FR16, FR18, FR19, FR20, FR21, FR24, FR26, FR29

**实现备注:** 包含 Tauri React TypeScript starter、基础 sidebar/tab 信息架构、工作区初始化、设置入口、状态枚举、错误模型、工作台 overview、上下文路由和返回路径。工作台快捷入口只能携带上下文跳转到对应 feature，不直接执行导入、删除、重分析、JSON 保存、索引重建等业务操作。

### Story 1.1: 建立应用壳、sidebar 导航与 `媒体导入` 命名

**FRs implemented:** FR13, FR24, FR26

As a 本地媒体处理用户,  
I want 打开 slicer 后看到稳定的桌面应用壳、左侧 sidebar 和明确的功能入口,  
So that 我可以从工作台进入媒体导入、媒体管理、模型分析、BM25 索引、搜索和设置，而不会被旧的“图片导入”命名或混乱入口干扰。

**Acceptance Criteria:**

**Given** 当前仓库已包含 BMad 规划文档和现有项目上下文  
**When** 开发者调整或初始化应用壳与前端主导航  
**Then** 应基于官方 Tauri React TypeScript starter 或已等价合并的 Tauri v2 + React + TypeScript + Vite 应用壳  
**And** 必须保留 `_bmad`、`_bmad-output`、`.agents`、`docs` 等既有规划和文档目录  
**And** 不得为了初始化应用壳覆盖既有规划工件

**Given** 用户启动 slicer 桌面应用  
**When** 应用完成首屏加载  
**Then** 默认显示 `工作台` 视图  
**And** 首屏不得是营销页、介绍页或空白占位页

**Given** 用户查看左侧 sidebar  
**When** sidebar 渲染主导航  
**Then** 应显示 `工作台`、`媒体导入`、`媒体管理`、`模型分析`、`一键导出`、`BM25 索引`、`搜索`、`设置` 等功能入口  
**And** 原 `图片导入` 文案不得再出现在用户可见导航、页面标题、按钮或空状态中

**Given** 用户点击 sidebar 中的任一功能入口  
**When** 目标功能尚未完整实现  
**Then** 应进入对应 route/tab 的可识别占位视图  
**And** 占位视图应使用对应功能名称，不得把多个功能混在工作台中

**Given** 用户使用键盘或屏幕阅读器浏览 sidebar  
**When** 焦点移动到导航项  
**Then** 每个导航项都应有清晰的可访问名称、焦点态和选中态  
**And** 当前选中 tab 不得只依赖颜色表达

**Given** 开发者检查前端结构  
**When** 查看工作台、媒体导入、媒体管理、模型分析、索引、搜索、设置的入口代码  
**Then** route-level feature 边界应清晰  
**And** 工作台不得直接实现媒体导入、媒体管理、模型分析、索引重建、搜索或导出业务逻辑

### Story 1.2: 选择、初始化并持久化本地工作区

**FRs implemented:** FR1, FR18

As a 本地媒体处理用户,  
I want 选择一个本地目录作为 slicer 工作区，并让应用记住和恢复这个工作区,  
So that 我的原始文件、页面图片、JSON、索引和任务状态都能稳定保存在我控制的本地位置。

**Acceptance Criteria:**

**Given** 用户首次启动 slicer 且尚未设置工作区  
**When** 用户进入工作台  
**Then** 工作台应显示“尚未选择工作区”的空状态  
**And** 应提供清晰的“选择工作区”入口  
**And** 媒体导入、媒体管理、模型分析、搜索、索引和导出入口可以展示，但应明确当前需要先选择工作区

**Given** 用户点击选择工作区  
**When** 用户选择一个本地目录  
**Then** 应用应校验该目录可访问、可写入  
**And** 路径包含中文、空格或较长文件名时必须正常处理

**Given** 用户选择的目录通过校验  
**When** 应用初始化工作区  
**Then** 应创建或确认存在 `originals/`、`pages/`、`metadata/`、`indexes/bm25/` 等标准子目录  
**And** 应创建或打开 `app.db`，只初始化本故事所需的最小工作区配置/版本信息，不提前创建未来故事才需要的全部业务表

**Given** 工作区初始化成功  
**When** 用户返回工作台或查看 sidebar 底部状态  
**Then** 应显示当前工作区可用状态和完整路径  
**And** 路径过长时应截断显示，并可通过 tooltip 或详情查看完整路径

**Given** 用户关闭并重新打开应用  
**When** 应用启动  
**Then** 应恢复上一次选择的工作区路径  
**And** 应重新校验标准目录和 `app.db` 是否存在  
**And** 缺失的可恢复目录应自动修复或给出明确修复入口

**Given** 用户选择另一个目录作为新工作区  
**When** 应用完成切换  
**Then** 应重新加载新工作区状态  
**And** 不得删除、迁移或覆盖旧工作区中的文件、数据库、页面图片、JSON 或索引

**Given** 工作区目录不可访问、权限不足、路径无效或初始化失败  
**When** 用户尝试选择或加载该工作区  
**Then** 应显示中文错误摘要、失败阶段和可恢复建议  
**And** 错误不得只写入日志或隐藏在诊断信息中

**Given** 开发者检查实现  
**When** 查看工作区初始化代码  
**Then** 文件路径必须通过 Rust path-safe API 处理  
**And** 不得通过前端字符串拼接推断 workspace 内部路径

### Story 1.3: 建立可恢复的工作区账本、状态枚举与错误记录

**FRs implemented:** FR19, FR20, FR21

As a 本地媒体处理用户,  
I want slicer 能把工作区、任务、媒体、页面和错误状态保存在本地账本中,  
So that 应用重启、任务失败或后续功能切换时，我仍能看到真实状态并恢复处理。

**Acceptance Criteria:**

**Given** 用户已经选择并初始化本地工作区  
**When** 应用打开或升级 `app.db`  
**Then** 应通过受控 migration 初始化本故事所需的最小 SQLite 账本结构  
**And** migration 必须可重复执行，不得破坏已有工作区数据

**Given** 应用需要表达文档/媒体生命周期  
**When** 开发者实现状态模型  
**Then** 文档/媒体状态至少应支持 `imported`、`converting`、`converted`、`conversion_failed`、`analyzing`、`analyzed`、`analysis_failed`、`indexed`、`index_failed`  
**And** 状态值在 SQLite、Rust DTO、Tauri event、JSONL 和前端展示中应保持 snake_case 一致

**Given** 应用需要表达页面生命周期  
**When** 开发者实现页面状态模型  
**Then** 页面状态至少应支持 `image_created`、`analysis_pending`、`analysis_running`、`analysis_succeeded`、`analysis_failed`、`indexed`  
**And** 页面状态不得只存在于 React 内存或 job event 中

**Given** 转换、分析、索引和后续批量重分析都属于长任务  
**When** 应用创建、更新或恢复任务状态  
**Then** 应使用持久化 job/task 记录作为 source of truth  
**And** Tauri events 只能作为 live UI hints，前端必须能通过查询 SQLite-backed service 恢复状态

**Given** 任一阶段发生错误  
**When** 应用记录错误  
**Then** 错误记录至少包含错误类型、错误摘要、发生阶段、关联文档或页面、是否可重试、最近一次发生时间和 correlation id  
**And** 用户可见错误必须是中文摘要，不得只暴露原始技术堆栈

**Given** 错误或诊断信息涉及模型配置、API key、token 或原始模型响应  
**When** 写入 SQLite、日志、job event、JSONL、前端状态或 HTTP 响应  
**Then** 必须执行敏感信息脱敏  
**And** API key、token 和完整未脱敏模型响应不得进入普通日志或导出 JSON

**Given** 用户关闭并重新打开应用  
**When** 工作区账本成功加载  
**Then** 工作台应能从账本恢复工作区可用性、任务数量、失败数量、媒体/页面数量和索引状态摘要  
**And** 即使暂无媒体，系统也应显示明确的零状态而不是空白或错误

**Given** `app.db` 缺失、损坏、版本不兼容或 migration 失败  
**When** 应用加载工作区  
**Then** 应显示结构化错误状态和可恢复建议  
**And** 不得删除已有 `originals/`、`pages/`、`metadata/` 或 `indexes/` 目录中的资产

**Given** 开发者检查实现  
**When** 查看 Tauri command、service、repository 和前端组件  
**Then** Tauri command 只调用 service，service 通过 repository 访问 SQLite  
**And** React 组件不得直接拼接文件路径、写 SQLite 或把前端状态当成权威账本

### Story 1.4: 设置页基础配置与隐私/依赖状态

**FRs implemented:** FR16

As a 本地媒体处理用户,  
I want 在设置页集中配置工作区、LibreOffice、模型连接、图片参数和并发参数,  
So that 我可以明确知道 slicer 的本地处理环境是否可用，以及哪些配置会影响媒体分析和隐私。

**Acceptance Criteria:**

**Given** 用户进入 `设置` tab  
**When** 设置页加载完成  
**Then** 应以分组形式展示工作区、LibreOffice、模型配置、图片参数、并发参数、隐私提示和本地 API/高级设置入口  
**And** 设置页不得呈现为无分组的长列表或开发工具式配置面板

**Given** 用户查看工作区设置分组  
**When** 当前工作区已选择  
**Then** 应显示当前工作区路径和状态  
**And** 应提供更改工作区入口，该入口复用 Story 1.2 的工作区选择与初始化能力

**Given** 用户查看 LibreOffice 设置分组  
**When** 用户输入 LibreOffice 可执行文件路径或点击自动检测  
**Then** 应校验该路径是否存在且可执行  
**And** 自动检测失败时应显示中文原因和手动配置建议

**Given** 用户查看模型配置分组  
**When** 用户填写 provider、base URL、custom endpoint、model name 和 API key  
**Then** provider、base URL、custom endpoint、model name 等非敏感配置应保存到 SQLite settings 或等价本地账本  
**And** API key 必须通过 OS credential storage 或 `keyring` 保存，不得明文写入 SQLite、JSONL、日志、错误详情或前端持久状态

**Given** API key 已保存  
**When** 用户重新打开设置页  
**Then** API key 字段应以掩码状态展示  
**And** 不得回显完整明文 key  
**And** 应提供更新或清除 key 的明确入口

**Given** 用户启用云端模型或自定义 HTTP endpoint  
**When** 设置页允许保存该配置  
**Then** 应显示隐私提示，说明页面图片会发送到用户配置的模型服务  
**And** 用户必须能在保存前看到该提示，而不是在实际分析失败后才发现

**Given** 用户查看图片参数和并发参数  
**When** 设置页加载默认值  
**Then** 默认图片 DPI 应为 144  
**And** 转换并发数、分析并发数应可配置并进行基础数值校验  
**And** 非法数值不得保存

**Given** 用户修改设置  
**When** 设置存在未保存变更  
**Then** 设置页应显示 dirty 状态  
**And** 保存时显示 saving 状态，成功后显示 saved 状态  
**And** 保存失败必须保留用户输入并显示字段级或分组级错误

**Given** 用户使用键盘或屏幕阅读器操作设置页  
**When** 焦点进入表单控件、按钮或敏感字段操作  
**Then** 每个输入项必须有明确 label  
**And** API key、自动检测、保存、清除等操作必须可键盘访问并有可访问名称

**Given** 开发者检查实现  
**When** 查看设置页、Tauri command 和 service 层代码  
**Then** 前端应通过 `lib/tauriClient.ts` 或等价封装调用设置命令  
**And** Tauri command 只做 DTO 转换和 service 调用，不得直接写 SQLite、keyring 或文件系统

### Story 1.5: 工作台 overview 展示与功能分流入口

**FRs implemented:** FR14, FR26, FR29

As a 本地媒体处理用户,  
I want 工作台只展示工作区概览、处理状态和下一步入口,  
So that 我可以快速判断当前能做什么，并跳转到正确功能页完成具体操作。

**Acceptance Criteria:**

**Given** 用户已选择可用工作区  
**When** 用户进入 `工作台` tab  
**Then** 工作台应展示当前工作区路径、工作区可用状态、媒体总量、页面总量、待处理数量、已分析数量、可搜索数量、失败数量和最近更新时间  
**And** 这些数据必须来自后端 service/SQLite 权威账本，不得由前端扫描文件系统或猜测状态

**Given** 工作区暂无媒体  
**When** 用户进入工作台  
**Then** 工作台应显示明确的零状态摘要  
**And** 应提供跳转到 `媒体导入` 的入口  
**And** 不得在工作台内直接嵌入完整导入 dropzone 或文件管理列表

**Given** 工作区已有媒体和任务  
**When** 工作台展示最近状态  
**Then** 可展示最近任务、最近失败、索引状态和模型配置状态的摘要  
**And** 摘要只用于快速判断，不替代 `媒体管理`、`模型分析`、`BM25 索引` 或 `搜索` 的完整操作界面

**Given** 用户想导入新媒体  
**When** 用户点击工作台中的导入快捷入口  
**Then** 应跳转到 `媒体导入` tab  
**And** 工作台不得直接打开文件选择器或处理导入业务逻辑

**Given** 用户想查看、筛选、删除、定位源文件或选择重分析对象  
**When** 用户点击工作台中的媒体管理入口或某个媒体状态摘要  
**Then** 应跳转到 `媒体管理` tab  
**And** 可携带筛选上下文，例如 `全部`、`失败`、`未分析`、`已完成` 或 `最近更新`

**Given** 用户想重新分析或修正 JSON  
**When** 用户点击工作台中的模型分析入口或分析失败摘要  
**Then** 应跳转到 `模型分析` tab  
**And** 可携带建议上下文，例如 `待分析`、`分析失败`、`需要重分析`  
**And** 工作台不得直接调用模型、保存 JSON 或创建重分析 job

**Given** 用户想查看索引状态或重建索引  
**When** 用户点击工作台中的 BM25 索引入口或索引状态摘要  
**Then** 应跳转到 `BM25 索引` tab 或对应索引管理视图  
**And** 工作台不得直接执行索引重建

**Given** 用户想搜索已有内容  
**When** 用户点击工作台中的搜索入口  
**Then** 应跳转到 `搜索` tab  
**And** 如果索引未就绪，应在目标搜索页显示索引状态，而不是在工作台伪造搜索结果

**Given** 工作台展示失败摘要  
**When** 用户点击失败摘要  
**Then** 应跳转到最相关的功能 tab，并携带失败阶段过滤条件  
**And** 失败原因详情和恢复动作应在目标功能页展示

**Given** 用户在窄窗口或较小桌面窗口查看工作台  
**When** 工作台布局响应式降级  
**Then** 状态摘要和快捷入口不得重叠、撑破或遮挡  
**And** 文案、按钮和状态 badge 应保持可读、可点击、可键盘访问

**Given** 开发者检查工作台实现  
**When** 查看工作台组件代码  
**Then** 工作台只应调用读取概览和导航上下文的 service/client  
**And** 不得包含导入、删除、模型分析、JSON 保存、索引重建或搜索执行的业务流程代码

### Story 1.6: 跨 tab 上下文跳转与返回路径

**FRs implemented:** FR29

As a 本地媒体处理用户,  
I want 从工作台、媒体管理、模型分析、搜索等 tab 之间跳转时保留上下文,  
So that 我不会在“查看状态、选择对象、重分析、编辑 JSON、返回原列表”这些流程中丢失选择和操作意图。

**Acceptance Criteria:**

**Given** 用户从工作台点击某个快捷入口  
**When** 应用跳转到目标 tab  
**Then** 跳转上下文应能携带来源 tab、建议操作、筛选条件或目标状态  
**And** 目标 tab 应基于上下文展示合适的默认视图，例如失败筛选、待分析筛选、索引状态或搜索可用性提示

**Given** 用户从工作台的失败摘要跳转  
**When** 失败来源是导入、转换、分析或索引  
**Then** 应跳转到最相关的功能 tab  
**And** 应携带失败阶段过滤条件  
**And** 目标 tab 应保留清晰返回工作台的路径

**Given** 用户在 `媒体管理` 中选择单个媒体、一个文档或一批页面  
**When** 用户点击 `重分析`  
**Then** 应跳转到 `模型分析` tab  
**And** 上下文必须包含选择对象类型、选择对象 ID 列表、来源筛选、来源 tab 和建议动作 `reanalyze`  
**And** 目标模型分析页不得依赖前端全局变量猜测选择内容

**Given** 用户从 `模型分析` 完成或取消重分析  
**When** 用户点击返回  
**Then** 应返回 `媒体管理` 中原先的筛选、选中范围或列表上下文  
**And** 如果原对象已被删除、移动或状态变化，应显示清晰提示，而不是静默失败

**Given** 用户从 `搜索` 页面查看某个结果并进入 JSON 编辑或重分析流程  
**When** 用户完成保存、取消或关闭编辑器  
**Then** 应能返回原搜索关键词、结果列表、选中结果和页面详情上下文  
**And** 最新 JSON 状态应通过后端查询刷新，而不是复用过期前端缓存

**Given** 路由上下文包含多个字段  
**When** 开发者实现上下文结构  
**Then** 应使用类型化的 navigation context，例如 `source_tab`、`return_to`、`action`、`selected_ids`、`filter`、`query`、`scroll_anchor`  
**And** 字段命名应与前端/后端 DTO 的 snake_case 约定保持一致，或在边界处明确转换

**Given** 用户刷新页面、应用重启或上下文已过期  
**When** 目标 tab 无法恢复原上下文  
**Then** 应降级为安全默认视图  
**And** 应显示轻量提示说明上下文已失效或需要重新选择对象

**Given** 开发者检查实现  
**When** 查看 routing、tab state 和 feature 组件  
**Then** 路由层只负责传递上下文和恢复视图状态  
**And** 不得在路由层执行导入、删除、模型调用、JSON 保存、索引重建或搜索业务逻辑

**Given** 使用键盘或屏幕阅读器操作跨 tab 流程  
**When** 用户完成跳转或返回  
**Then** 焦点应落在目标 tab 的主标题或关键操作区  
**And** 返回按钮或上下文提示应有清晰的可访问名称

### Epic 2: 媒体导入与媒体资产管理

用户可以在 `媒体导入` 中导入图片和文档媒体，系统完成重复识别、类型校验、PDF/Office 转页面图片、图片哈希命名、页面记录和 JSONL artifact。用户随后在 `媒体管理` 中查看、搜索、筛选、预览、定位源文件、删除和选择待重分析对象。

**FRs covered:** FR2, FR3, FR4, FR7, FR25

**实现备注:** 包含 `媒体导入` 命名后的导入入口、图片/文档文件类型支持、originals 登记、PDF/Office 转换、页面图片原子写入、SQLite 与 JSONL 一致性、媒体管理列表、状态筛选、页面详情、源文件定位、删除和重分析选择入口。媒体管理必须从 service/SQLite 权威账本读取，不得直接扫描文件系统作为主数据来源。

### Story 2.1: 媒体导入入口、拖拽/选择与类型预检

**FRs implemented:** FR2

As a 本地媒体处理用户,  
I want 在 `媒体导入` tab 中拖拽或选择图片/文档媒体文件，并立即看到类型预检结果,  
So that 我可以在正式入库或转换前知道哪些文件会被接收、哪些文件被拒绝以及原因。

**Acceptance Criteria:**

**Given** 用户尚未选择工作区  
**When** 用户进入 `媒体导入` tab  
**Then** 页面应显示需要先选择工作区的状态  
**And** 应提供跳转到工作台或设置页选择工作区的入口  
**And** 不得允许创建导入任务

**Given** 用户已选择可用工作区  
**When** 用户进入 `媒体导入` tab  
**Then** 页面应显示拖拽区域和文件选择按钮  
**And** 页面标题、空状态、按钮和提示文案必须统一使用 `媒体导入`，不得出现旧的 `图片导入`

**Given** 用户拖拽或选择多个文件  
**When** 文件进入预检流程  
**Then** 系统应逐项识别文件名、扩展名、大小、媒体类型和基础可读性  
**And** 支持类型至少包括 PNG、JPG/JPEG、WEBP、PDF、PPT、PPTX、DOC、DOCX

**Given** 文件类型不受支持、文件不可读或路径无效  
**When** 预检完成  
**Then** 应逐项显示拒绝状态和中文原因  
**And** 被拒绝文件不得进入入库、转换、分析或索引流程

**Given** 文件通过基础类型预检  
**When** 页面展示待导入列表  
**Then** 每个文件应显示文件名、类型、大小、路径摘要和当前状态  
**And** 长文件名、中文路径或含空格路径不得撑破布局

**Given** 用户确认开始导入  
**When** 本故事范围内提交导入  
**Then** 应仅创建后续入库流程所需的导入请求或 job 入口  
**And** 不得在本故事中实现 PDF/Office 转换、模型分析、BM25 索引或 JSON 编辑

**Given** 用户拖拽文件到窗口  
**When** 拖拽进入、离开或释放  
**Then** 应显示清晰的 drop overlay 或接收态反馈  
**And** 拖拽不是唯一入口，文件选择按钮必须始终可用

**Given** 用户使用键盘或屏幕阅读器操作媒体导入页  
**When** 焦点进入拖拽区域、文件选择按钮、待导入列表或拒绝项  
**Then** 应有明确 label、焦点态和可读状态  
**And** 拒绝原因不得只通过颜色表达

**Given** 开发者检查实现  
**When** 查看 `媒体导入` 页面代码  
**Then** 前端只负责文件选择、预检展示和调用导入 service/client  
**And** 不得在 React 组件中直接写 SQLite、复制文件到 workspace、执行转换或创建页面图片

### Story 2.2: 原始媒体登记、哈希去重与 originals 存储

**FRs implemented:** FR2

As a 本地媒体处理用户,  
I want slicer 在导入媒体时登记原始文件、计算哈希并识别重复项,  
So that 我的工作区不会因为重复导入、长文件名或路径差异产生混乱记录。

**Acceptance Criteria:**

**Given** 用户提交通过预检的媒体/文档文件  
**When** 导入 service 接收请求  
**Then** 应为每个文件创建持久化导入 job 或等价任务记录  
**And** job 状态必须写入 SQLite 权威账本，不能只存在于前端内存

**Given** 文件进入入库流程  
**When** 系统读取源文件内容  
**Then** 应计算原始文件内容哈希  
**And** 哈希计算失败时应记录可恢复错误，不得创建不完整 document/media 记录

**Given** 文件内容哈希与已有原始文件重复  
**When** 系统完成重复识别  
**Then** 应将该项标记为 duplicate 或等价状态  
**And** 界面应向用户说明可跳过、重新转换或后续重新分析，而不是静默复制第二份原文件

**Given** 文件不是重复项  
**When** 系统登记原始文件  
**Then** 应创建 `documents` 或等价媒体记录，包含 `document_id`、原始文件名、原始路径、文件类型、文件大小、原始哈希和导入时间  
**And** 路径和文件名字段必须支持中文、空格和长文件名

**Given** 原始文件需要进入工作区  
**When** 系统写入 `originals/`  
**Then** 应使用受控文件名或目录结构保存/登记原始文件  
**And** 写入应采用临时文件加原子替换或等价安全策略，避免半写文件被当成有效源文件

**Given** 原始文件已登记  
**When** 用户重启应用并进入媒体管理或工作台摘要  
**Then** 应能从 SQLite 恢复该文件的导入状态、来源信息和后续处理状态  
**And** 不得依赖重新扫描 `originals/` 才知道文件存在

**Given** 入库过程中发生权限、磁盘空间、读取或写入错误  
**When** 系统记录失败  
**Then** 应保存失败阶段、中文摘要、是否可重试和关联文件信息  
**And** 失败不得删除已成功入库的其他文件

**Given** 开发者检查实现  
**When** 查看 import service、repository 和 artifact store  
**Then** 原始文件登记、哈希、复制和数据库写入必须通过 service 编排  
**And** Tauri command 不得直接复制文件或写 SQLite

### Story 2.3: 图片媒体直接入库为页面资产

**FRs implemented:** FR2, FR4, FR7

As a 本地媒体处理用户,  
I want PNG、JPG/JPEG、WEBP 等图片媒体可以直接成为可预览的页面资产,  
So that 单张图片不需要经过文档转换流程，也能进入后续模型分析和搜索链路。

**Acceptance Criteria:**

**Given** 用户导入 PNG、JPG/JPEG 或 WEBP 图片  
**When** 原始文件登记完成  
**Then** 系统应将该图片作为单页媒体处理  
**And** 应创建 `document_id` 或等价媒体记录，并创建对应的单页 `page_record`

**Given** 图片媒体进入页面资产生成流程  
**When** 系统写入页面图片  
**Then** 页面资产应进入 `pages/<document_id>/` 或等价受控目录  
**And** 如需统一页面格式，输出页面资产应规范为 PNG 或明确记录格式策略

**Given** 页面图片写入成功  
**When** 系统创建页面记录  
**Then** 页面记录应包含 `page_id`、`document_id`、`page_number = 1`、`total_pages = 1`、`image_hash`、`image_path` 和初始页面状态  
**And** 初始页面状态应至少能表达已生成图片且待分析

**Given** 图片内容与已有页面图片相同  
**When** 系统计算 `image_hash`  
**Then** 相同图片内容应生成相同 `image_hash`  
**And** 不得因原始文件名不同覆盖或破坏已有页面资产

**Given** 图片文件损坏、无法解码或格式伪装  
**When** 页面资产生成失败  
**Then** 对应媒体应进入可恢复失败状态  
**And** 错误摘要应说明图片无法读取或解码，而不是显示泛化失败

**Given** 图片入库完成  
**When** 用户进入媒体管理  
**Then** 应能看到该图片的缩略图、文件名、类型、页数为 1、当前分析状态和源文件信息  
**And** 不得要求用户先执行模型分析才能查看图片页面

**Given** 本故事完成后  
**When** 开发者检查能力边界  
**Then** 不应在本故事中实现 PDF/Office 转换、模型分析、BM25 索引或 JSON 编辑  
**And** 后续模型分析应只消费已创建的页面记录和页面图片资产

### Story 2.4: PDF/Office 转换为页面 PNG

**FRs implemented:** FR3

As a 本地媒体处理用户,  
I want PDF、PPT/PPTX、DOC/DOCX 等文档媒体能被转换为逐页 PNG 页面资产,  
So that 文档资料也能像图片一样进入页面级分析、管理和检索流程。

**Acceptance Criteria:**

**Given** 用户导入 PDF 文件  
**When** 转换 job 开始  
**Then** 系统应逐页渲染 PDF 为 PNG 页面图片  
**And** 1 页、30 页和 300 页 PDF 均应能生成正确数量的页面记录或明确失败原因

**Given** 用户导入 PPT、PPTX、DOC 或 DOCX 文件  
**When** 转换 job 开始  
**Then** 系统应先通过本机 LibreOffice headless 转换为中间 PDF  
**And** 再将中间 PDF 逐页渲染为 PNG 页面图片

**Given** LibreOffice 未配置、未检测到或不可执行  
**When** 用户导入 Office 文档  
**Then** 转换应失败为可恢复状态  
**And** 错误摘要应提示用户到设置页配置 LibreOffice 路径

**Given** LibreOffice 转换超时、失败或输出异常  
**When** 系统记录失败  
**Then** 应保存 stderr 摘要或诊断信息的安全截断版本  
**And** 不得把完整敏感路径、API key 或无关系统秘密写入普通错误展示

**Given** 文档转换属于长任务  
**When** 转换正在执行  
**Then** GUI 不得卡死  
**And** 用户应能看到后台 job 的阶段、进度、成功页数、失败页数和最近更新时间

**Given** 转换过程中部分页面成功、部分页面失败  
**When** 系统保存结果  
**Then** 成功页面应保留并可预览  
**And** 失败状态应明确标记为部分完成或可恢复失败，不得把整批成功结果全部回滚

**Given** 转换完成  
**When** 页面图片已写入 workspace  
**Then** 每页必须创建对应 `page_record`，包含来源文档、页码、总页数、图片路径和页面状态  
**And** 不得等到模型分析完成才创建页面记录

**Given** 开发者检查实现  
**When** 查看 conversion provider 边界  
**Then** LibreOffice 和 PDF renderer 必须通过 provider/service 封装  
**And** React 组件和 Tauri command 不得直接 spawn LibreOffice 或写页面图片文件

### Story 2.5: 页面图片哈希命名、原子写入与 JSONL 一致性

**FRs implemented:** FR4, FR7

As a 本地媒体处理用户,  
I want 页面图片、SQLite 记录和 JSONL artifact 始终保持一致,  
So that 应用重启、导入中断或文件重复时，我的页面资产不会丢失、错乱或被覆盖。

**Acceptance Criteria:**

**Given** 任一页面图片生成完成  
**When** 系统写入页面资产  
**Then** 应基于图片内容计算 `image_hash`  
**And** 页面图片文件名应使用 `<image_hash>.png` 或等价稳定命名策略

**Given** 两个页面图片内容相同  
**When** 系统计算内容哈希  
**Then** 应生成相同 `image_hash`  
**And** `image_hash` 只代表图片内容身份，不得替代页面 occurrence 的 `page_id`

**Given** 页面图片写入文件系统  
**When** 写入过程执行  
**Then** 应使用临时文件加原子 rename 或等价安全机制  
**And** 半写文件不得被登记为有效页面图片

**Given** 页面图片写入成功  
**When** 系统提交数据库状态  
**Then** SQLite 中的 `page_record`、`image_asset` 或等价记录必须引用真实存在的图片路径  
**And** 文件路径必须由 Rust path-safe API 生成和校验

**Given** 系统需要输出可读元数据  
**When** 页面记录或分析状态变化  
**Then** 应从 SQLite 权威账本生成或更新 `metadata/pages.jsonl` 或等价 JSONL artifact  
**And** JSONL 不得成为业务读取的 source of truth

**Given** JSONL 写入正在执行  
**When** 写入失败或应用中断  
**Then** 不得破坏上一版可用 JSONL  
**And** 系统应记录可恢复错误，并允许后续重建 JSONL

**Given** 应用启动或用户触发一致性检查  
**When** 系统比较 SQLite、页面图片和 JSONL artifact  
**Then** 应能识别缺失图片、孤儿图片、过期 JSONL 或无法解析 JSONL  
**And** 识别结果应进入可诊断状态，不得静默忽略

**Given** 开发者检查实现  
**When** 查看 artifact store 与 repository  
**Then** SQLite 提交、图片写入和 JSONL 更新必须有清晰顺序和失败保护  
**And** 不得让前端直接构造 workspace 内部路径或写 JSONL

### Story 2.6: 媒体管理列表、筛选、详情与工作台模块迁移

**FRs implemented:** FR25

As a 本地媒体处理用户,  
I want 在 `媒体管理` tab 中集中查看和筛选已导入媒体、文档和页面资产,  
So that 工作台保持清爽分流，而具体媒体管理操作集中在正确位置完成。

**Acceptance Criteria:**

**Given** 用户已选择工作区并已有导入记录  
**When** 用户进入 `媒体管理` tab  
**Then** 应展示媒体/文档列表，包含缩略图或占位图、文件名、类型、页数、导入状态、转换状态、分析状态、索引状态和最近更新时间  
**And** 列表数据必须来自后端 service/SQLite 权威账本

**Given** 用户需要定位媒体  
**When** 用户在媒体管理中搜索或筛选  
**Then** 应支持按文件名、路径摘要、类型、导入/转换/分析/索引状态和失败状态筛选  
**And** 筛选不得隐藏当前批次或工作区摘要中的失败总数

**Given** 用户选择某个媒体/文档记录  
**When** 详情区域加载  
**Then** 应展示来源文件、页数、页面缩略图、状态历史摘要、失败摘要和可用操作入口  
**And** 页面缩略图加载失败时应显示降级状态，不得显示破图

**Given** 当前工作台仍存在旧文档管理模块  
**When** 完成本故事迁移  
**Then** 旧文档/媒体管理列表、搜索筛选、详情、源文件、删除和重分析入口应从工作台迁移到 `媒体管理`  
**And** 工作台只保留摘要和跳转入口

**Given** 用户从工作台点击媒体数量、失败摘要或最近任务  
**When** 跳转到 `媒体管理`  
**Then** 媒体管理应接收筛选上下文并展示对应列表  
**And** 应提供返回工作台的清晰路径

**Given** 用户在宽屏和窄窗口查看媒体管理  
**When** 列表、详情和缩略图布局响应式调整  
**Then** 长文件名、长路径、多个状态 badge 和按钮不得撑破布局  
**And** 缩略图和操作按钮应有稳定尺寸

**Given** 用户使用键盘或屏幕阅读器操作媒体管理  
**When** 焦点进入列表项、筛选器、详情区或操作按钮  
**Then** 应有可访问名称、焦点态和选中态  
**And** 状态不得只依赖颜色表达

**Given** 开发者检查实现  
**When** 查看媒体管理 feature  
**Then** 媒体管理只能调用查询、筛选和上下文操作 service  
**And** 不得直接扫描 workspace 文件系统作为主数据来源

### Story 2.7: 媒体管理操作、删除、源文件定位与重分析选择上下文

**FRs implemented:** FR25, FR27

As a 本地媒体处理用户,  
I want 在 `媒体管理` 中对单个或批量媒体执行查看源文件、删除和选择重分析等操作,  
So that 我能维护媒体资产，并把需要重新分析的对象准确送入 `模型分析` 模块。

**Acceptance Criteria:**

**Given** 用户在媒体管理中查看某个媒体/文档  
**When** 用户点击 `源文件`  
**Then** 应打开或定位到该媒体在工作区中的原始文件位置  
**And** 如果源文件缺失，应显示可恢复错误和一致性检查入口

**Given** 用户选择删除单个媒体/文档  
**When** 用户点击删除  
**Then** 应显示确认 dialog，说明将影响的原始文件登记、页面记录、页面图片、JSONL、索引状态和分析结果  
**And** 未确认前不得删除任何记录或文件

**Given** 用户确认删除  
**When** 删除执行成功  
**Then** 应通过 service 层更新 SQLite 权威账本、相关 artifact 状态和索引 stale/needs_rebuild 状态  
**And** 工作台、媒体管理和搜索页后续查询不得继续显示已删除项为有效结果

**Given** 删除过程中部分文件或记录无法删除  
**When** 系统记录失败  
**Then** 应保留可恢复错误详情  
**And** 不得让数据库指向已经不存在且未标记异常的页面图片

**Given** 用户在媒体管理中选择一个图片、一个文档或多项媒体  
**When** 用户点击 `重分析`  
**Then** 应跳转到 `模型分析` tab  
**And** navigation context 必须包含选择对象类型、ID 列表、来源 tab、来源筛选、建议动作 `reanalyze` 和选择数量

**Given** 用户选择了不适合重分析的对象，例如没有页面图片、已删除项或状态不完整项  
**When** 用户点击 `重分析`  
**Then** 应显示逐项不可重分析原因  
**And** 不得把无效对象带入模型分析队列

**Given** 用户选择大量对象执行批量重分析  
**When** 媒体管理构建跳转上下文  
**Then** 应清楚展示选择数量和对象范围  
**And** 不得在媒体管理中直接创建模型调用、直接编辑 JSON 或执行分析 job

**Given** 用户使用键盘或屏幕阅读器操作媒体管理动作  
**When** 焦点进入 `查看页面`、`源文件`、`删除`、`重分析` 等操作  
**Then** 每个操作都应具备 tooltip、aria-label 和可键盘触发路径  
**And** 删除等危险操作必须有确认流程

**Given** 开发者检查实现  
**When** 查看媒体操作代码  
**Then** 删除、源文件定位和重分析上下文必须通过 typed service/client 执行  
**And** 媒体管理不得直接调用模型 provider、写 JSON、重建索引或绕过 service 修改 SQLite

### Epic 3: 模型分析、重分析与可信 JSON 修正

用户可以配置模型服务，对媒体页面执行模型分析，生成可信 `page_analysis_v1` JSON。当结果不准确时，用户可以从 `媒体管理` 选择单个或批量对象跳转到 `模型分析`，添加自定义提示词重分析，或直接进入 JSON 编辑/微调流程。所有保存都经过 schema 校验、敏感信息检查、原子写入和索引刷新/标记流程。

**FRs covered:** FR5, FR6, FR17, FR22, FR23, FR27, FR28

**实现备注:** 包含模型 provider/endpoint 抽象、keyring 密钥保存、API key redaction、分析任务编排、schema 校验、`analysis_results` 入库、页面级 JSON 生成、单个/批量重分析、从媒体管理带入选择上下文、自定义提示词、手动 JSON 编辑、原子保存、当前有效结果指针和分析结果来源审计。

### Story 3.1: 模型分析模块基础视图与媒体管理上下文接收

**FRs implemented:** FR27, FR28

As a 本地媒体处理用户,  
I want `模型分析` tab 能接收从 `媒体管理` 带来的单个或批量重分析上下文,  
So that 我能清楚知道即将分析哪些媒体页面，并在同一个模型分析模块中继续选择自定义提示词或 JSON 微调流程。

**Acceptance Criteria:**

**Given** 用户直接进入 `模型分析` tab  
**When** 当前没有从媒体管理带入选择上下文  
**Then** 页面应显示模型分析总览，包括模型配置状态、待分析页面数量、分析失败数量、最近分析任务和可用入口  
**And** 不得显示空白页面或要求用户先从媒体管理进入

**Given** 用户尚未选择工作区  
**When** 用户进入 `模型分析` tab  
**Then** 页面应显示需要先选择工作区的状态  
**And** 应提供跳转到工作台或设置页选择工作区的入口  
**And** 不得允许创建分析或重分析任务

**Given** 用户从 `媒体管理` 中选择一个图片、一个文档或一批页面  
**When** 用户点击 `重分析` 并跳转到 `模型分析`  
**Then** `模型分析` tab 应展示选择上下文摘要  
**And** 摘要至少包含对象类型、选择数量、来源范围、待分析页数、已有 JSON 数量、失败项数量和来源 tab

**Given** 带入的选择上下文包含无效对象  
**When** `模型分析` tab 解析上下文  
**Then** 应逐项标记无效原因，例如页面图片缺失、对象已删除、没有可分析页面或状态不完整  
**And** 无效对象不得进入可提交的分析队列

**Given** 带入的选择上下文有效  
**When** 页面展示可执行动作  
**Then** 应至少提供 `使用默认提示词重新分析`、`添加自定义提示词重新分析`、`编辑 JSON` 或等价入口  
**And** 这些入口只负责进入对应流程，不得在 Story 3.1 中直接调用模型或保存 JSON

**Given** 模型配置尚未完成  
**When** 用户查看可执行动作  
**Then** 模型调用相关动作应 disabled 或显示配置提示  
**And** JSON 查看/编辑入口如果已有有效 JSON，可继续进入编辑流程，不应被模型配置缺失完全阻断

**Given** 开发者检查实现  
**When** 查看 `模型分析` feature 代码  
**Then** 页面应通过 typed navigation context 和 service 查询加载对象状态  
**And** 不得从媒体管理的 React 内存读取选择项，也不得直接扫描文件系统或直接调用模型 provider

### Story 3.2: 模型 Provider、Endpoint 与密钥安全调用准备

**FRs implemented:** FR5

As a 本地媒体处理用户,  
I want 模型分析模块能安全读取我的 provider、endpoint、model name 和 API key 配置,  
So that 页面图片可以发送到我指定的模型服务，同时密钥不会泄露。

**Acceptance Criteria:**

**Given** 用户已在设置页保存模型配置  
**When** 模型分析模块加载配置状态  
**Then** 应显示 provider、base URL、custom endpoint、model name 和密钥是否已配置  
**And** 不得显示完整 API key

**Given** 用户尚未配置 provider、endpoint、model name 或 API key  
**When** 用户尝试发起模型分析  
**Then** 应阻止提交并显示缺失配置项  
**And** 应提供跳转设置页的入口

**Given** 后端准备调用模型 provider  
**When** service 读取密钥  
**Then** API key 必须从 OS credential storage 或 `keyring` 读取  
**And** 不得从 SQLite 普通字段、前端状态、JSONL 或日志中读取完整明文 key

**Given** 模型 provider 请求失败  
**When** 系统记录错误  
**Then** 错误摘要必须进行密钥、token 和敏感 header 脱敏  
**And** 不得将完整请求头、完整 API key 或完整未脱敏响应写入普通日志

**Given** 用户启用云端模型或自定义 HTTP endpoint  
**When** 首次在模型分析模块发起调用  
**Then** 应确认或展示隐私提示，说明页面图片会发送到用户配置的模型服务  
**And** 用户应能取消操作

**Given** 开发者添加新的 provider 实现  
**When** 查看模型调用层  
**Then** provider 必须通过 `ModelProvider` 或等价 trait/adapter 接入  
**And** 分析 service 不得硬编码为单一模型供应商

### Story 3.3: `page_analysis_v1` Schema、Prompt 契约与校验器

**FRs implemented:** FR17

As a 本地媒体处理用户,  
I want 模型输出必须符合统一的 `page_analysis_v1` JSON schema,  
So that 只有结构可信的页面元数据会进入本地账本、JSONL 和搜索索引。

**Acceptance Criteria:**

**Given** 开发者查看模型分析模块  
**When** 定义页面分析 schema  
**Then** `page_analysis_v1` 至少应包含 `page_id`、`image_hash`、`image_path`、source、analysis、retrieval、model 和 `schema_version`  
**And** schema 字段命名应使用 snake_case

**Given** 系统组装默认分析 prompt  
**When** 调用模型 provider  
**Then** prompt 应明确要求模型返回符合 `page_analysis_v1` 的 JSON  
**And** 不得要求模型返回未定义结构或纯自然语言摘要

**Given** 模型返回 JSON 内容  
**When** schema validator 执行校验  
**Then** 应验证 JSON 语法、必填字段、字段类型、schema version 和敏感字段  
**And** 校验失败不得写入当前有效分析结果

**Given** 模型返回非法 JSON、字段类型错误、缺失字段或未知 schema version  
**When** 校验失败  
**Then** 应产生结构化校验错误，包含用户可读摘要和调试所需的 JSON path 或字段名  
**And** 错误详情不得包含 API key、token 或完整未脱敏模型响应

**Given** 后续存在默认分析、提示词重分析和手动 JSON 编辑  
**When** 它们保存候选 JSON  
**Then** 必须复用同一个 schema validator  
**And** 不得为不同入口维护互相不一致的 JSON 校验逻辑

### Story 3.4: 可信分析结果保存管线、版本指针与索引刷新标记

**FRs implemented:** FR6, FR17, FR22, FR23, FR28

As a 本地媒体处理用户,  
I want 每一次有效分析结果保存都可追溯、可回退且不会破坏上一版 JSON,  
So that 模型分析、提示词重分析和手动 JSON 微调都能安全更新当前有效结果。

**Acceptance Criteria:**

**Given** 某个页面已有或即将产生有效 `page_analysis_v1` JSON  
**When** 保存管线接收候选结果  
**Then** 必须先通过 JSON 语法、schema 和敏感信息校验  
**And** 未通过校验的候选结果不得写入为 current result

**Given** 候选结果通过校验  
**When** 保存管线写入 SQLite  
**Then** 应创建新的 `analysis_result` 或等价版本记录  
**And** 记录至少包含 `page_id`、schema version、source_type、provider/model 摘要、结果 JSON、创建时间和 base result 引用

**Given** 新结果写入成功  
**When** 系统更新当前有效结果  
**Then** 应通过 current pointer 或等价机制指向新结果  
**And** 旧结果应保留为历史版本，不得被静默覆盖

**Given** 保存过程中 SQLite、页面 JSON artifact、JSONL 或索引刷新任一步失败  
**When** 保存管线回报失败  
**Then** 上一版当前有效 JSON 必须保持可用  
**And** 失败应记录为可恢复错误

**Given** 当前有效 JSON 更新成功  
**When** 页面 JSON/JSONL artifact 需要同步  
**Then** 应从 SQLite 权威账本生成或更新相关 artifact  
**And** 如果无法局部更新索引，应将索引标记为 stale/needs_rebuild

**Given** 保存来源可能是模型生成、提示词重分析或手动编辑  
**When** 写入分析结果  
**Then** 应记录 `source_type`，例如 `model_generated`、`prompt_regenerated`、`manual_edit`  
**And** 审计信息不得包含敏感密钥或完整未脱敏请求

### Story 3.5: 单页图片模型分析端到端纵切片

**FRs implemented:** FR6

As a 本地媒体处理用户,  
I want 对单个已生成页面图片执行模型分析并得到可信 JSON,  
So that 我可以验证从页面图片到结构化元数据的核心闭环可用。

**Acceptance Criteria:**

**Given** 工作区中存在一个 `image_created` 或 `analysis_pending` 页面  
**When** 用户在模型分析模块选择单页分析  
**Then** 系统应创建持久化分析 job  
**And** job 创建后命令应快速返回，不得阻塞 GUI

**Given** 分析 job 开始执行  
**When** service 组装模型请求  
**Then** 请求应包含页面图片引用、页面来源信息、默认 prompt 和 `page_analysis_v1` 输出约束  
**And** 前端不得直接调用模型 endpoint

**Given** 模型 provider 返回候选 JSON  
**When** schema validator 校验通过  
**Then** 应通过 Story 3.4 的可信保存管线保存结果  
**And** 页面状态应更新为 `analysis_succeeded` 或等价成功状态

**Given** 分析成功  
**When** 用户回到模型分析页或媒体管理页  
**Then** 应能看到该页面已有当前有效 JSON、分析时间、provider/model 摘要和可继续重分析/编辑入口

**Given** 模型返回非法 JSON、超时或网络错误  
**When** 单页分析失败  
**Then** 页面不得产生新的当前有效 JSON  
**And** 失败应记录为可重试分析错误

### Story 3.6: 新页面批量分析、单文档分析与进度恢复

**FRs implemented:** FR6

As a 本地媒体处理用户,  
I want 对新页面、单个文档或一批页面执行批量模型分析，并能看到进度和恢复状态,  
So that 大量媒体可以稳定进入可搜索 JSON 状态。

**Acceptance Criteria:**

**Given** 工作区中存在多个待分析页面  
**When** 用户发起批量分析  
**Then** 系统应创建持久化批量分析 job  
**And** job 应记录总页数、待处理页数、成功页数、失败页数和当前阶段

**Given** 用户选择单个文档执行分析  
**When** 文档包含多页页面图片  
**Then** 系统应只分析该文档下尚未分析或用户明确要求重跑的页面  
**And** 不得重复分析已经有效且未被标记重跑的页面

**Given** 批量分析正在执行  
**When** 用户切换 tab、关闭窗口或重新打开应用  
**Then** 前端应能通过后端查询恢复 job 状态  
**And** Tauri events 只能作为 live hints，不得成为 source of truth

**Given** 批量分析部分成功、部分失败  
**When** job 完成  
**Then** 成功页面应保留当前有效 JSON  
**And** 失败页面应记录失败原因和可重试状态

**Given** 批量对象数量较大  
**When** 页面展示进度  
**Then** GUI 不得卡死  
**And** 状态更新应清楚展示处理中、成功、失败和剩余数量

### Story 3.7: 分析失败处理、单页重试与安全诊断

**FRs implemented:** FR6

As a 本地媒体处理用户,  
I want 分析失败时能看到原因并重试单页或失败项,  
So that 临时网络、模型输出或配置问题不会让整批媒体卡住。

**Acceptance Criteria:**

**Given** 模型调用发生超时、网络错误、HTTP 错误、provider 错误、非法 JSON 或 schema 校验失败  
**When** 系统记录失败  
**Then** 应记录错误类型、阶段、用户可读摘要、retryable、关联 `page_id` 和 correlation id  
**And** 错误不得只显示在日志中

**Given** 页面分析失败且错误可重试  
**When** 用户点击单页重试  
**Then** 系统应创建新的分析 job 或 job attempt  
**And** 不得覆盖上一版有效 JSON

**Given** 批量分析存在失败项  
**When** 用户选择重试全部失败项  
**Then** 系统应只重新提交失败页面  
**And** 已成功页面不得被无故重跑

**Given** 失败涉及 API key、token、请求 header 或原始模型响应  
**When** 错误写入日志、SQLite、job event 或 UI  
**Then** 必须脱敏  
**And** 原始模型响应只允许保存安全截断摘要

**Given** 用户修复设置后返回模型分析页  
**When** 查看失败项  
**Then** 应能从失败详情进入重试  
**And** 应保留原失败上下文和最近发生时间

### Story 3.8: 自定义提示词重分析，支持单个与批量上下文

**FRs implemented:** FR22, FR27, FR28

As a 本地媒体处理用户,  
I want 对单个或批量媒体页面添加自定义提示词重新分析,  
So that 我可以把人工判断或纠错意图反馈给模型，生成更准确的结构化 JSON。

**Acceptance Criteria:**

**Given** 用户从媒体管理带入单个或批量重分析上下文  
**When** 用户选择 `添加自定义提示词重新分析`  
**Then** 页面应展示自定义提示词输入区、选择对象摘要、当前 JSON 状态和预计重分析页数  
**And** 用户应能取消并返回来源上下文

**Given** 用户输入自定义提示词  
**When** 用户提交重分析  
**Then** 系统应创建持久化 prompt regeneration job  
**And** 请求应包含选择对象、base analysis result、用户提示词和必要页面上下文引用

**Given** 后端组装 prompt regeneration 请求  
**When** 调用模型 provider  
**Then** prompt 应包含原页面图片、当前有效页面 JSON、用户提示词和 `page_analysis_v1` 输出约束  
**And** 模型 provider、schema validator 和保存管线必须与普通分析流程一致

**Given** 模型返回候选 JSON  
**When** 候选通过校验  
**Then** 可展示候选结果供确认，或按产品设定直接通过可信保存管线保存  
**And** 保存后应记录 `source_type = prompt_regenerated` 或等价审计字段

**Given** 批量提示词重分析包含多个页面  
**When** 部分页面成功、部分页面失败  
**Then** 成功页面更新当前有效 JSON  
**And** 失败页面保留旧 JSON 并显示可重试原因

**Given** 用户提示词包含敏感信息或过长内容  
**When** 系统保存审计信息  
**Then** 只允许保存必要、可审计且非敏感的提示词摘要或引用  
**And** 不得把 API key、token 或完整敏感提示内容写入普通日志

### Story 3.9: JSON 编辑/微调、全屏校验与可信保存

**FRs implemented:** FR23, FR28

As a 本地媒体处理用户,  
I want 直接查看、编辑和校验页面当前有效 JSON,  
So that 当模型输出只需要人工微调时，我可以不重新调用模型也能修正页面元数据。

**Acceptance Criteria:**

**Given** 页面已有当前有效 `page_analysis_v1` JSON  
**When** 用户在模型分析或搜索上下文中选择 `编辑 JSON`  
**Then** 应打开全屏或近全屏 JSON 编辑器  
**And** 编辑器应载入当前有效 JSON、页面图片预览或明确的页面上下文

**Given** 用户修改 JSON  
**When** 用户点击校验  
**Then** 系统应执行 JSON 语法校验和 `page_analysis_v1` schema 校验  
**And** 错误应显示行列、字段名或 JSON path

**Given** 用户输入非法 JSON 或 schema-invalid JSON  
**When** 用户尝试保存  
**Then** 不得写入 SQLite、JSONL、索引或当前有效结果  
**And** 编辑器应保留用户输入以便修正

**Given** 用户输入合法且通过校验的 JSON  
**When** 用户保存  
**Then** 应通过 Story 3.4 的可信保存管线保存  
**And** 保存结果应记录 `source_type = manual_edit` 或等价审计字段

**Given** 保存过程中发生冲突，例如当前结果已被其他流程更新  
**When** 用户提交旧 base result 的编辑  
**Then** 系统应提示冲突并阻止静默覆盖  
**And** 用户应能重新加载最新 JSON 后再编辑

**Given** 用户关闭、取消或离开编辑器  
**When** 存在未保存草稿  
**Then** 应提示保存、放弃或取消离开  
**And** 未确认保存前不得隐式写入

**Given** JSON 保存成功  
**When** 用户返回来源页面  
**Then** 应显示最新当前有效 JSON  
**And** 搜索索引应局部更新或标记 stale/needs_rebuild 并提示用户

**Given** 用户使用键盘或屏幕阅读器操作编辑器  
**When** 焦点进入编辑区、错误列表、保存、校验、取消或关闭按钮  
**Then** 应有明确可访问名称和焦点路径  
**And** Escape/关闭行为不得造成未确认修改丢失

### Epic 4: 本地 BM25 索引与页面级搜索体验

用户可以基于可信页面 JSON 构建或重建本地 BM25 索引，并在搜索页按中文/英文关键词检索页面。搜索结果返回页面图片、来源、页码、分数和 JSON；索引不可用、重建中、失败或 stale 时，界面给出明确状态与恢复入口。

**FRs covered:** FR8, FR9, FR10, FR12, FR15

**实现备注:** 包含 `SearchProvider` 抽象、`TantivyBm25SearchProvider`、中文 tokenizer/analyzer 策略、索引 build 目录与 active index 原子切换、重建失败保护、搜索结果 DTO、GUI 搜索页、图片预览、JSON 查看、索引状态和 stale/needs_rebuild 提示。查询接口不能硬编码成只支持 BM25。

### Story 4.1: SearchProvider 抽象与索引就绪状态

**FRs implemented:** FR12, FR15

As a 本地媒体处理用户,  
I want slicer 的搜索能力通过清晰的检索抽象和索引状态来表达,  
So that 我知道当前是否可以搜索、索引是否可用，以及未来是否能扩展到其他检索方式。

**Acceptance Criteria:**

**Given** 开发者查看检索层实现  
**When** 阅读搜索相关 service 和 provider 代码  
**Then** 应存在 `SearchProvider` 或等价抽象  
**And** 查询接口不得硬编码为只能支持 BM25

**Given** 当前工作区没有可用索引  
**When** 用户进入搜索页或工作台索引摘要  
**Then** 应显示明确的 `未建索引`、`索引中`、`可搜索`、`需要重建` 或 `索引失败` 状态  
**And** 不得让用户把“索引中”误解为“没有结果”

**Given** 索引正在构建或重建  
**When** 用户查看搜索入口  
**Then** 搜索页应显示索引状态和预计不可用提示  
**And** 不得伪造可以搜索的假结果

**Given** 索引失败或 stale  
**When** 用户查看搜索页  
**Then** 应显示可读的中文状态和重建入口  
**And** 状态信息应来自 SQLite/索引状态，而不是前端内存推断

**Given** 用户搜索页尚未输入关键词  
**When** 页面加载  
**Then** 应展示搜索输入框、索引状态和空状态提示  
**And** 空状态不得阻断页面预览、索引说明或状态查看

**Given** 开发者新增未来检索实现  
**When** 该实现接入系统  
**Then** 应只需实现 `SearchProvider` 边界  
**And** 不得要求改写搜索页 UI、SQLite 权威账本或上层 service 语义

### Story 4.2: Tantivy BM25 Provider 与中文 analyzer

**FRs implemented:** FR8, FR12

As a 本地媒体处理用户,  
I want slicer 使用能处理中文内容的本地 BM25 provider,  
So that 我搜索中文文件名、标题、摘要、可见文字、主题或关键词时能命中相关页面。

**Acceptance Criteria:**

**Given** MVP 默认检索 provider 为 BM25  
**When** 开发者实现 provider  
**Then** 应接入 `TantivyBm25SearchProvider` 或等价本地 BM25 实现  
**And** provider 必须实现 `SearchProvider` 边界

**Given** 页面分析结果包含中文内容  
**When** provider 构建可检索文本  
**Then** 应显式使用中文 tokenizer/analyzer 策略，例如 jieba、字符 n-gram 或混合策略  
**And** 具体策略应有样例测试验证中文关键词可命中

**Given** 页面 JSON 包含标题、摘要、可见文字、主题、关键词和来源文件名  
**When** 构建索引文档  
**Then** 索引文本应包含这些字段  
**And** 不得只索引标题或纯文件名

**Given** provider 接收空查询、超长查询或特殊字符查询  
**When** 执行搜索前校验  
**Then** 应返回结构化错误或安全空结果  
**And** 不得 panic 或破坏 active index

**Given** 后续可能加入 qmd、wiki search、向量或混合检索  
**When** 查看 BM25 实现  
**Then** BM25 细节应封装在 provider 内  
**And** SearchService 和 UI 不应依赖 Tantivy 内部类型

### Story 4.3: 从可信分析结果构建初始索引

**FRs implemented:** FR8

As a 本地媒体处理用户,  
I want 基于当前有效页面 JSON 构建第一个可用搜索索引,  
So that 已分析的媒体页面可以进入本地可搜索状态。

**Acceptance Criteria:**

**Given** 工作区中存在通过 schema 校验的当前有效页面 JSON  
**When** 用户触发构建索引或系统自动构建初始索引  
**Then** IndexService 应从 SQLite 权威账本读取当前有效分析结果  
**And** 不得把 `metadata/pages.jsonl`、前端状态或未校验模型输出当作 source of truth

**Given** 页面分析结果进入索引构建  
**When** provider 创建索引文档  
**Then** 每个索引文档应保留 `page_id`、`document_id`、页码、图片引用和页面 JSON 引用所需字段  
**And** 搜索命中后必须能追溯到原始文档、页码、页面图片和结构化 JSON

**Given** 索引构建是长任务  
**When** 构建开始  
**Then** 应创建持久化 index job  
**And** GUI 应能显示阶段、进度、已索引页数、失败页数和最近更新时间

**Given** 部分页面缺少当前有效 JSON 或页面图片  
**When** 构建索引  
**Then** 缺失项应被跳过或记录为可诊断状态  
**And** 不得阻止其他有效页面进入索引

**Given** 初始索引构建成功  
**When** 用户查看工作台、媒体管理或搜索页  
**Then** 应显示 `可搜索` 状态和可搜索页面数量  
**And** 搜索页应允许提交查询

### Story 4.4: 全量索引重建、active index 原子切换与失败保护

**FRs implemented:** FR10

As a 本地媒体处理用户,  
I want 可以安全地全量重建 BM25 索引,  
So that JSON 修正、删除或批量变化后，搜索状态可以恢复且不会破坏上一版可用索引。

**Acceptance Criteria:**

**Given** 用户触发全量索引重建  
**When** 重建 job 开始  
**Then** 应在 `indexes/bm25/build-<id>/` 或等价临时 build 目录中构建新索引  
**And** 不得直接写入 active index 目录

**Given** 新索引构建完成  
**When** 系统验证索引可读且包含预期元数据  
**Then** 应原子切换 active index 指针或目录  
**And** 切换成功后再更新 SQLite 中的 active index 状态

**Given** 重建过程中失败  
**When** job 进入失败状态  
**Then** 上一个可用 active index 必须保持可用  
**And** 错误应记录失败阶段、中文摘要、retryable 和 correlation id

**Given** 页面 JSON 被手动编辑、提示词重生成或媒体被删除  
**When** 搜索索引无法局部更新  
**Then** 应将索引标记为 stale/needs_rebuild  
**And** 搜索页应提示用户重建或等待后台重建

**Given** 重建索引过程中存在页面图片和页面 JSON artifact  
**When** job 执行  
**Then** 应用不得删除原图片或页面 JSON  
**And** 重建失败不得破坏 SQLite、JSONL 或页面资产

### Story 4.5: SearchService 查询结果 DTO 与可追溯返回

**FRs implemented:** FR9

As a 本地媒体处理用户,  
I want 搜索结果返回页面 JSON、图片地址、来源文档、页码和相关分数,  
So that 我可以快速确认命中的页面是否就是我要找的内容。

**Acceptance Criteria:**

**Given** active index 可用  
**When** 用户提交查询和 limit  
**Then** SearchService 应调用当前 SearchProvider 执行搜索  
**And** 查询接口不得绕过 provider 直接读取 Tantivy 或文件系统

**Given** provider 返回命中结果  
**When** SearchService 组装 DTO  
**Then** 每条结果应包含 `page_id`、`document_id`、来源文件名、页码、页面图片地址/引用、相关分数和页面 JSON 摘要  
**And** 不允许只返回文本片段而缺少图片或来源信息

**Given** 搜索结果需要返回页面 JSON  
**When** SearchService 查询当前有效结果  
**Then** 应返回通过 `page_analysis_v1` 校验的当前有效 JSON 或其安全摘要  
**And** JSON 中不得包含 API key、token、完整原始模型响应或未脱敏错误详情

**Given** 图片文件缺失或路径不可访问  
**When** SearchService 组装结果  
**Then** 应返回结构化 image_missing 状态或错误摘要  
**And** 不得由前端猜测文件路径

**Given** 查询时索引不可用、stale 或正在重建  
**When** 用户发起搜索  
**Then** 应返回明确的搜索不可用状态  
**And** 不得把不可用状态伪装成“无结果”

### Story 4.6: 搜索页输入、结果列表、图片预览与 JSON 查看

**FRs implemented:** FR9, FR15

As a 本地媒体处理用户,  
I want 在搜索页输入关键词并查看结果列表、页面图片预览和页面 JSON,  
So that 我可以用桌面 GUI 检索并审查媒体页面。

**Acceptance Criteria:**

**Given** 用户进入 `搜索` tab 且 active index 可用  
**When** 用户输入中文或英文关键词并执行搜索  
**Then** 页面应展示按相关性排序的结果列表  
**And** 每条结果应显示标题/摘要、来源文档、页码、分数和基础命中信息

**Given** 用户选择某条搜索结果  
**When** 搜索页加载详情区域  
**Then** 应显示页面图片预览  
**And** 图片引用必须来自后端 service 返回的受控路径/URL，不得由前端猜测文件布局

**Given** 用户需要查看结构化结果  
**When** 用户打开 JSON 查看区域  
**Then** 应展示该页面的当前有效 `page_analysis_v1` JSON 或安全格式化视图  
**And** JSON 中不得包含 API key、token、完整原始模型响应或未脱敏错误详情

**Given** 用户发现页面 JSON 中的人物、主题、关键词或摘要不准确  
**When** 用户在 JSON 查看区选择自定义提示词重分析或编辑 JSON  
**Then** 搜索页应跳转或打开 Epic 3 的对应入口，并保留当前搜索结果和页面选择上下文  
**And** 搜索页不得直接调用模型、写 SQLite、写 JSONL 或更新索引

**Given** 搜索没有匹配结果  
**When** active index 可用且查询完成  
**Then** 应显示明确的无结果空状态  
**And** 无结果状态应区别于索引未就绪、索引中、索引失败和搜索错误

**Given** 用户在窄窗口查看搜索页  
**When** 三栏布局无法完整展示  
**Then** 结果列表、页面预览和 JSON 详情应降级为可切换面板或上下结构  
**And** JSON viewer 不得撑破页面

**Given** 用户使用键盘或屏幕阅读器操作搜索页  
**When** 焦点进入搜索框、结果列表、页面预览、JSON 查看或跳转操作  
**Then** 应支持键盘提交、结果选择、返回和详情查看  
**And** 状态、选中项和错误不能只依赖颜色表达

### Epic 5: Localhost HTTP API 与外部自动化访问

用户或本机自动化工具可以通过默认仅监听 `127.0.0.1` 的 HTTP API 查询健康状态、搜索结果、页面详情、文档详情，并通过受保护接口触发索引重建。GUI 和 API 使用同一套 Rust service layer，避免业务逻辑分叉。

**FRs covered:** FR11

**实现备注:** 包含 axum API、`GET /health`、`GET /search`、`GET /pages/{page_id}`、`GET /documents/{document_id}`、`POST /indexes/rebuild`、token 保护、统一成功/错误响应结构，以及 Tauri commands 和 HTTP handlers 共享 application services。

### Story 5.1: Localhost API Server 生命周期、健康检查与安全监听

**FRs implemented:** FR11

As a 本地自动化用户,  
I want slicer 提供默认只监听本机地址的 HTTP API 服务和健康检查,  
So that 我可以让其他本地工具安全地确认 slicer 是否可用，而不把工作区暴露到公网。

**Acceptance Criteria:**

**Given** 用户已选择工作区并启用本地 API  
**When** 应用启动 API server  
**Then** API 默认必须绑定 `127.0.0.1` 或 localhost 等本机地址  
**And** 不得默认监听 `0.0.0.0` 或公网地址

**Given** 用户尚未启用本地 API  
**When** 应用启动  
**Then** API server 可以保持关闭  
**And** 设置页或状态区域应显示 API 当前启用/禁用状态

**Given** API server 已启用  
**When** 本地工具请求 `GET /health`  
**Then** 应返回服务状态、工作区是否 ready、索引状态和 API 版本  
**And** 响应不得包含 API key、token、完整路径中的敏感片段或内部堆栈

**Given** 工作区未选择、不可访问或账本加载失败  
**When** 本地工具请求 `GET /health`  
**Then** 应返回结构化状态，说明 workspace 不可用  
**And** HTTP 响应不得伪装成完全可用状态

**Given** API server 启动失败，例如端口占用或绑定失败  
**When** 用户查看设置页或工作台状态  
**Then** 应显示中文错误摘要和可恢复建议  
**And** 不得影响 GUI 的本地工作区浏览和媒体管理能力

**Given** 用户修改 API 启用状态、端口或 bind address  
**When** 保存设置  
**Then** 应校验配置安全性  
**And** 如允许非 localhost 绑定，必须明确提示风险并要求用户显式确认

**Given** 开发者检查实现  
**When** 查看 API server 和 route handler  
**Then** 应使用 `axum` 或架构批准的 HTTP 框架  
**And** route handler 只负责 HTTP DTO 映射并调用 shared service layer，不得直接访问 SQLite、文件系统、模型 provider 或索引 provider

### Story 5.2: 搜索、页面和文档只读 API

**FRs implemented:** FR11

As a 本地自动化用户,  
I want 通过 localhost API 查询搜索结果、页面 JSON 和文档页面列表,  
So that 我可以把 slicer 的页面级知识资产接入其他本地工具链。

**Acceptance Criteria:**

**Given** API server 已启用且工作区 ready  
**When** 本地工具请求 `GET /search?q={query}&limit={n}`  
**Then** API 应返回与 GUI 搜索相同语义的搜索结果  
**And** 每条结果应包含 `page_id`、`document_id`、score、image_path 或受控图片引用、页面 JSON 摘要和来源信息

**Given** 查询参数缺失、limit 非法或 query 为空  
**When** 请求 `GET /search`  
**Then** API 应返回结构化错误或安全空结果  
**And** 不得 panic 或返回内部堆栈

**Given** 本地工具请求 `GET /pages/{page_id}`  
**When** 页面存在且有当前有效 JSON  
**Then** API 应返回完整页面 JSON、图片地址/引用、来源文档、页码、分析状态和索引状态  
**And** JSON 不得包含 API key、token、完整原始模型响应或未脱敏错误详情

**Given** 本地工具请求 `GET /documents/{document_id}`  
**When** 文档存在  
**Then** API 应返回文档元数据、原始文件信息摘要、页数、导入/转换/分析/索引状态和页面列表  
**And** 页面列表应能追溯到 `page_id`、页码和页面图片

**Given** 请求的 page 或 document 不存在、已删除或不可访问  
**When** API 处理请求  
**Then** 应返回结构化 404 或等价错误  
**And** 错误响应应包含 code、message、stage、retryable 和 correlation_id

**Given** HTTP route handler 处理只读请求  
**When** 查看实现  
**Then** handler 必须调用 SearchService、Page/Document service 或等价 shared services  
**And** 不得直接读取 SQLite row、拼接文件路径或扫描 workspace 文件系统

### Story 5.3: 受 token 保护的索引重建 API

**FRs implemented:** FR11

As a 本地自动化用户,  
I want 通过受保护的 localhost API 触发索引重建,  
So that 外部本地工具可以在 JSON 更新后请求 slicer 刷新搜索能力，同时避免未授权进程随意触发重任务。

**Acceptance Criteria:**

**Given** API server 已启用  
**When** 本地工具请求 `POST /indexes/rebuild`  
**Then** 该端点必须要求本地 token 或等价保护机制  
**And** 缺失或无效 token 应返回结构化未授权错误

**Given** token 有效且工作区 ready  
**When** 请求 `POST /indexes/rebuild`  
**Then** API 应创建持久化索引重建 job  
**And** 响应应返回 job_id、初始状态和可查询的状态信息

**Given** 索引重建已在运行  
**When** 再次请求重建  
**Then** API 应返回当前 job 状态或明确拒绝重复提交  
**And** 不得启动多个互相破坏 active index 的并发重建

**Given** token 需要重置  
**When** 用户在设置页触发 token reset  
**Then** 应生成新的本地 token  
**And** 旧 token 应失效

**Given** API 错误日志或响应涉及 token  
**When** 写入日志、错误或 HTTP 响应  
**Then** 不得输出完整 token  
**And** 只能显示安全截断或不可逆摘要

**Given** route handler 触发索引重建  
**When** 查看实现  
**Then** handler 只能调用 IndexService 或 JobService  
**And** 不得直接写索引目录、active index 指针或 SQLite job row

### Story 5.4: API 合同测试、错误响应与敏感信息脱敏

**FRs implemented:** FR11

As a 本地自动化用户,  
I want slicer 的 localhost API 有稳定合同、清晰错误和敏感信息保护,  
So that 外部工具可以可靠集成，而不会因为内部实现变化或错误泄露造成风险。

**Acceptance Criteria:**

**Given** API 返回成功响应  
**When** 本地工具调用任一 endpoint  
**Then** 成功响应应使用稳定对象结构，例如 `{ "data": ... }` 或架构批准的等价格式  
**And** 字段命名应使用 snake_case

**Given** API 返回错误响应  
**When** 请求参数错误、资源不存在、工作区不可用、索引不可用、token 无效或内部处理失败  
**Then** 错误响应应使用稳定结构，例如 `{ "error": { "code", "message", "stage", "retryable", "correlation_id" } }`  
**And** 不得返回 Rust panic、内部堆栈或未脱敏 provider 响应

**Given** API 响应包含路径、模型信息、错误详情或索引状态  
**When** 响应序列化  
**Then** 应进行敏感信息脱敏  
**And** API key、token、完整模型请求 header 和完整未脱敏模型响应不得出现在响应中

**Given** 开发者修改 API DTO 或 route  
**When** 运行 API contract tests  
**Then** 测试应覆盖 `GET /health`、`GET /search`、`GET /pages/{page_id}`、`GET /documents/{document_id}` 和 `POST /indexes/rebuild`  
**And** 测试应校验成功响应、错误响应、token 保护、索引不可用和资源不存在场景

**Given** GUI 和 HTTP API 都调用相同业务能力  
**When** 比较 GUI 搜索和 `GET /search` 结果  
**Then** 二者应使用同一 SearchService 语义  
**And** 不得出现 GUI 与 API 返回不同业务规则的情况

**Given** API 合同示例需要给外部工具参考  
**When** 文档或测试 fixture 生成示例  
**Then** 示例应包含搜索结果、页面详情、文档详情、健康检查和错误响应  
**And** 示例数据不得包含真实 API key、token 或用户私密路径

### Change Story CC-2026-06-10: 媒体管理与工作台路由调整

**Scope:** Correct Course / Moderate Change

**Impacted Epics:** Epic 1, Epic 2, Epic 3, Epic 4

**Primary FRs:** FR-013, FR-014, FR-015, FR-016, FR-017, FR-018

As a 本地媒体处理用户,  
I want `图片导入` 被统一更名为 `媒体导入`，并新增 `媒体管理` tab，把工作台中的具体媒体管理操作迁移出去,  
So that 工作台只负责状态概览与功能分流，而导入、管理、重分析和 JSON 微调都发生在职责清晰的功能模块中。

**Acceptance Criteria:**

**Given** 用户查看左侧 sidebar 或页面标题  
**When** 应用渲染主导航和功能页面  
**Then** 应显示 `媒体导入`，不得再显示旧的 `图片导入` 文案  
**And** 主导航应包含 `媒体管理`

**Given** 用户进入 `工作台`  
**When** 工作区已选择并存在媒体、任务或失败状态  
**Then** 工作台只展示工作区状态、媒体/页面/失败/索引摘要、最近任务摘要和快捷入口  
**And** 工作台不得直接承载完整导入 dropzone、媒体/文档管理列表、删除、模型调用、JSON 保存、索引重建、搜索执行或导出执行

**Given** 用户进入 `媒体导入`  
**When** 用户拖拽或选择文件  
**Then** 页面应支持图片和文档媒体的导入入口、类型预检、导入提交和导入反馈  
**And** 不得在 `媒体导入` 中承载删除、源文件定位、JSON 编辑或重分析选择

**Given** 用户进入 `媒体管理`  
**When** 工作区存在已导入媒体、文档或页面资产  
**Then** 页面应展示列表、搜索筛选、详情、状态、缩略图、源文件定位、删除和重分析选择入口  
**And** 数据必须来自后端 service/SQLite 权威账本，不得直接扫描 workspace 文件系统作为主数据源

**Given** 用户在 `媒体管理` 中选择单个媒体、单个文档、单页或批量对象  
**When** 用户点击 `重分析`  
**Then** 应跳转到 `模型分析` tab  
**And** navigation context 必须包含 `source_tab`、`return_to`、`action = reanalyze`、选择对象类型、ID 列表、来源筛选和选择数量  
**And** `媒体管理` 不得直接调用模型 provider、创建模型请求或保存 JSON

**Given** `模型分析` 接收到重分析上下文  
**When** 页面展示可执行动作  
**Then** 应展示选择对象摘要、当前 JSON 状态、预计重分析页数，并提供默认重分析、自定义提示词重分析和 JSON 编辑/微调入口

**Given** 用户完成、取消或关闭重分析/JSON 编辑流程  
**When** 用户返回来源页面  
**Then** 应返回 `媒体管理` 中原先的筛选、选中范围或列表上下文  
**And** 最新 JSON 状态应通过后端查询刷新，而不是复用过期前端缓存

**Given** 开发者检查实现  
**When** 查看 route/tab state、feature 组件和 typed client  
**Then** 工作台、媒体导入、媒体管理、模型分析的职责边界必须清晰  
**And** 具体业务逻辑必须通过对应 service/client 调用，不得塞入工作台或路由层
