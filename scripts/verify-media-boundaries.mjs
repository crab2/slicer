import { readFileSync, existsSync } from "node:fs";
import { join } from "node:path";

const root = process.cwd();
const checks = [];

function read(path) {
  return readFileSync(join(root, path), "utf8");
}

function expect(name, condition, detail) {
  checks.push({ name, ok: Boolean(condition), detail });
}

const navigation = read("src/app/navigation.ts");
const appShell = read("src/app/AppShell.tsx");
const workbench = read("src/features/workbench/WorkbenchPage.tsx");
const analysis = read("src/features/analysis/AnalysisPage.tsx");
const tauriClient = read("src/lib/tauriClient.ts");

const mediaImportExists = existsSync(join(root, "src/features/media-import/MediaImportPage.tsx"));
const mediaManagementExists = existsSync(
  join(root, "src/features/media-management/MediaManagementPage.tsx"),
);
const mediaImport = mediaImportExists
  ? read("src/features/media-import/MediaImportPage.tsx")
  : "";
const mediaManagement = mediaManagementExists
  ? read("src/features/media-management/MediaManagementPage.tsx")
  : "";

expect(
  "导航不再暴露旧文案",
  !navigation.includes("图片导入") && !appShell.includes("图片导入"),
  "navigation/AppShell must not contain 图片导入",
);
expect(
  "导航包含媒体导入与媒体管理",
  navigation.includes('"mediaImport"') &&
    navigation.includes('"mediaManagement"') &&
    navigation.includes("媒体导入") &&
    navigation.includes("媒体管理"),
  "ViewId/navigationItems must include mediaImport and mediaManagement labels",
);
expect(
  "typed navigation context 已定义",
  navigation.includes("ReanalysisNavigationContext") &&
    navigation.includes("source_tab") &&
    navigation.includes("return_to") &&
    navigation.includes("selected_kind") &&
    navigation.includes("selected_ids") &&
    navigation.includes("selection_count"),
  "navigation context must include route and restore fields",
);
expect(
  "AppShell 注册媒体管理并传递 context",
  appShell.includes("MediaManagementPage") &&
    appShell.includes("navigationContext") &&
    appShell.includes("onNavigateWithContext") &&
    appShell.includes("onReturnToSource"),
  "AppShell must own cross-tab context and register media management",
);
expect(
  "工作台不再执行业务命令",
  !workbench.includes("openImportDialog") &&
    !workbench.includes("deleteDocument") &&
    !workbench.includes("reanalyzeDocument") &&
    !workbench.includes("reanalyzeFailedPages") &&
    !workbench.includes("analyzePage(") &&
    !workbench.includes("exportMedia(") &&
    !workbench.includes("IndexStatusPanel"),
  "WorkbenchPage should be an overview/routing surface only",
);
expect(
  "工作台保留 service-backed 摘要读取",
  workbench.includes("listDocuments") &&
    workbench.includes("listWorkbenchPages") &&
    workbench.includes("listJobs") &&
    workbench.includes("getIndexStatus"),
  "WorkbenchPage should still read summary data through tauriClient",
);
expect(
  "媒体导入 feature 存在且复用统一导入",
  mediaImportExists &&
    mediaImport.includes("媒体导入") &&
    mediaImport.includes("SUPPORTED_EXTENSIONS") &&
    mediaImport.includes("isSupportedFileType") &&
    mediaImport.includes("getUnsupportedReason") &&
    mediaImport.includes("importMultipleFiles") &&
    !mediaImport.includes("DocumentList") &&
    !mediaImport.includes("deleteDocument") &&
    !mediaImport.includes("reanalyze"),
  "MediaImportPage must import supported media only and avoid management actions",
);
expect(
  "统一媒体文件选择器支持图片和文档",
  tauriClient.includes("openMediaImportDialog") &&
    tauriClient.includes("SUPPORTED_EXTENSIONS.map") &&
    tauriClient.includes("媒体"),
  "tauriClient should expose openMediaImportDialog from fileValidation extensions",
);
expect(
  "媒体管理 feature 承载管理和重分析路由",
  mediaManagementExists &&
    mediaManagement.includes("媒体管理") &&
    mediaManagement.includes("listDocuments") &&
    mediaManagement.includes("listWorkbenchPages") &&
    mediaManagement.includes("deleteDocument") &&
    mediaManagement.includes("revealDocumentInFolder") &&
    mediaManagement.includes("onNavigateWithContext") &&
    mediaManagement.includes("selected_ids") &&
    !mediaManagement.includes("reanalyzeDocument(") &&
    !mediaManagement.includes("reanalyzeFailedPages(") &&
    !mediaManagement.includes("analyzePage("),
  "MediaManagementPage should manage assets but route reanalysis through context only",
);
expect(
  "模型分析接收并展示重分析上下文",
  analysis.includes("navigationContext") &&
    analysis.includes("ReanalysisContextSummary") &&
    analysis.includes("reanalyzeDocument") &&
    analysis.includes("onReturnToSource") &&
    analysis.includes("自定义提示词") &&
    analysis.includes("JSON 编辑"),
  "AnalysisPage must receive context, summarize it, and own reanalysis actions",
);

const failed = checks.filter((check) => !check.ok);
for (const check of checks) {
  console.log(`${check.ok ? "PASS" : "FAIL"} ${check.name}`);
  if (!check.ok) {
    console.log(`  ${check.detail}`);
  }
}

if (failed.length > 0) {
  console.error(`\n${failed.length} media boundary check(s) failed.`);
  process.exit(1);
}
