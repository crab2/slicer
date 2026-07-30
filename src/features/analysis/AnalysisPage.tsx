import { useEffect, useMemo, useState } from "react";
import { Button } from "../../components/common/Button";
import { ErrorMessage } from "../../components/common/ErrorMessage";
import { StatusBadge } from "../../components/common/StatusBadge";
import { PrivacyNotice } from "../settings/components/PrivacyNotice";
import { tauriClient } from "../../lib/tauriClient";
import type { NavigationContext } from "../../app/navigation";
import type {
  AnalysisBatchResultDto,
  AnalysisResultDto,
  AppErrorDto,
  DocumentDto,
  ModelConfigurationStatusDto,
  PageWorkbenchDto,
} from "../../types/app";

interface AnalysisPageProps {
  workspaceReady: boolean;
  isActive: boolean;
  navigationContext: NavigationContext | null;
  onOpenSettings: () => void;
  onReturnToSource: () => void;
}

interface ReanalysisSummary {
  selectedKind: string;
  selectionCount: number;
  sourceTab: string;
  selectedLabels: string[];
  analyzablePages: number;
  existingJsonPages: number;
  failedItems: number;
  retryFailedOnly: boolean;
}

interface AnalysisDisplayError {
  title: string;
  message: string;
  details?: string | null;
  correlationId?: string | null;
}

export function AnalysisPage({
  workspaceReady,
  isActive,
  navigationContext,
  onOpenSettings,
  onReturnToSource,
}: AnalysisPageProps) {
  const [modelStatus, setModelStatus] = useState<ModelConfigurationStatusDto | null>(null);
  const [isModelStatusLoading, setIsModelStatusLoading] = useState(false);
  const [showPrivacyNotice, setShowPrivacyNotice] = useState(false);
  const [isAcceptingPrivacy, setIsAcceptingPrivacy] = useState(false);
  const [analysisReadyMessage, setAnalysisReadyMessage] = useState<string | null>(null);
  const [analysisError, setAnalysisError] = useState<AnalysisDisplayError | null>(null);
  const [isBatchAnalyzing, setIsBatchAnalyzing] = useState(false);
  const [isReanalyzing, setIsReanalyzing] = useState(false);
  const [pendingAction, setPendingAction] = useState<"new-pages" | "reanalysis" | null>(null);
  const [documents, setDocuments] = useState<DocumentDto[]>([]);
  const [pagesByDocument, setPagesByDocument] = useState<Record<string, PageWorkbenchDto[]>>({});

  const analysisStats = computeAnalysisStats(documents, pagesByDocument);
  const reanalysisSummary = useMemo(
    () =>
      navigationContext?.action === "reanalyze"
        ? buildReanalysisSummary(navigationContext, documents, pagesByDocument)
        : null,
    [navigationContext, documents, pagesByDocument],
  );
  const analysisConfigured =
    modelStatus?.configured &&
    (!modelStatus.requires_privacy_notice || modelStatus.privacy_notice_accepted);

  async function refreshModelStatus() {
    setIsModelStatusLoading(true);
    try {
      setModelStatus(await tauriClient.getModelConfigurationStatus());
    } catch {
      setModelStatus(null);
    } finally {
      setIsModelStatusLoading(false);
    }
  }

  async function refreshDocuments() {
    if (!workspaceReady) {
      setDocuments([]);
      setPagesByDocument({});
      return;
    }
    try {
      const docs = await tauriClient.listDocuments();
      setDocuments(docs);
      const pagesMap: Record<string, PageWorkbenchDto[]> = {};
      for (const doc of docs) {
        try {
          pagesMap[doc.document_id] = await tauriClient.listWorkbenchPages(doc.document_id);
        } catch {
          pagesMap[doc.document_id] = [];
        }
      }
      setPagesByDocument(pagesMap);
    } catch {
      setDocuments([]);
      setPagesByDocument({});
    }
  }

  useEffect(() => {
    if (!workspaceReady || !isActive) {
      return;
    }
    void refreshModelStatus();
    void refreshDocuments();
  }, [workspaceReady, isActive, navigationContext]);

  useEffect(() => {
    if (!workspaceReady || !isActive || (!isBatchAnalyzing && !isReanalyzing)) {
      return;
    }
    const timer = window.setInterval(() => {
      void refreshDocuments();
    }, 2000);
    return () => window.clearInterval(timer);
  }, [workspaceReady, isActive, isBatchAnalyzing, isReanalyzing]);

  async function handleAnalysisEntry() {
    setAnalysisError(null);
    setAnalysisReadyMessage(null);
    if (!modelStatus?.configured) {
      onOpenSettings();
      return;
    }
    if (modelStatus.requires_privacy_notice && !modelStatus.privacy_notice_accepted) {
      setPendingAction("new-pages");
      setShowPrivacyNotice(true);
      return;
    }
    await executeAnalyzeNewPages();
  }

  async function handleDefaultReanalysis() {
    setAnalysisError(null);
    setAnalysisReadyMessage(null);
    if (!navigationContext || navigationContext.action !== "reanalyze") {
      return;
    }
    if (!modelStatus?.configured) {
      onOpenSettings();
      return;
    }
    if (modelStatus.requires_privacy_notice && !modelStatus.privacy_notice_accepted) {
      setPendingAction("reanalysis");
      setShowPrivacyNotice(true);
      return;
    }
    await executeReanalysis();
  }

  async function executeAnalyzeNewPages() {
    setIsBatchAnalyzing(true);
    setAnalysisError(null);
    try {
      const result = await tauriClient.analyzeNewPages();
      setAnalysisReadyMessage(formatBatchMessage("新页面批量分析完成", result));
      setAnalysisError(getBatchResultError(result));
      await refreshDocuments();
    } catch (error) {
      setAnalysisError(extractError(error));
      await refreshDocuments();
    } finally {
      setIsBatchAnalyzing(false);
    }
  }

  async function executeReanalysis() {
    if (!navigationContext || navigationContext.action !== "reanalyze") {
      return;
    }

    setIsReanalyzing(true);
    setAnalysisError(null);
    try {
      if (navigationContext.selected_kind === "document" || navigationContext.selected_kind === "document_batch") {
        const results: AnalysisBatchResultDto[] = [];
        for (const documentId of navigationContext.selected_ids) {
          results.push(
            await (navigationContext.retry_failed_only
              ? tauriClient.reanalyzeFailedPages(documentId)
              : tauriClient.reanalyzeDocument(documentId)),
          );
        }
        setAnalysisReadyMessage(
          formatCombinedBatchMessage(
            navigationContext.retry_failed_only ? "失败项重试完成" : "默认重分析完成",
            results,
          ),
        );
        setAnalysisError(
          results.map(getBatchResultError).find((error) => error !== null) ?? null,
        );
      } else {
        const results: AnalysisResultDto[] = [];
        for (const pageId of navigationContext.selected_ids) {
          const selectedPage = findWorkbenchPage(pageId, pagesByDocument);
          if (selectedPage?.status === "structured") {
            throw new Error("结构化 PDF 按图片模块分析，请从媒体管理选择整个文档重分析。");
          }
          results.push(await tauriClient.analyzePage(pageId));
        }
        setAnalysisReadyMessage(formatPageAnalysisMessage("页面重分析完成", results));
      }
      await refreshDocuments();
    } catch (error) {
      setAnalysisError(extractError(error));
      await refreshDocuments();
    } finally {
      setIsReanalyzing(false);
    }
  }

  async function handlePrivacyConfirm() {
    setIsAcceptingPrivacy(true);
    try {
      await tauriClient.acceptPrivacyNotice();
      setShowPrivacyNotice(false);
      const status = await tauriClient.getModelConfigurationStatus();
      setModelStatus(status);
      const action = pendingAction;
      setPendingAction(null);
      if (action === "new-pages") {
        await executeAnalyzeNewPages();
      } else if (action === "reanalysis") {
        await executeReanalysis();
      } else {
        setAnalysisReadyMessage("隐私提示已确认。可以开始模型分析。");
      }
    } catch (error) {
      setAnalysisError(extractError(error));
    } finally {
      setIsAcceptingPrivacy(false);
    }
  }

  if (!workspaceReady) {
    return (
      <div className="panel">
        <p className="muted-copy">请先选择工作区。</p>
      </div>
    );
  }

  return (
    <div className="page-grid analysis-page">
      <section className="panel panel-wide">
        <div className="panel-header">
          <div>
            <p className="eyebrow">页面分析</p>
            <h2>模型分析</h2>
            <p className="muted-copy">
              {formatAnalysisOverview(analysisStats)}
              {!analysisConfigured && modelStatus?.configured
                ? " 请先确认隐私提示。"
                : ""}
            </p>
          </div>
          <StatusBadge
            tone={
              modelStatus?.configured
                ? modelStatus.privacy_notice_accepted || !modelStatus.requires_privacy_notice
                  ? "success"
                  : "warning"
                : "neutral"
            }
          >
            {isModelStatusLoading
              ? "检查中"
              : modelStatus?.configured
                ? "已配置"
                : "未配置"}
          </StatusBadge>
        </div>
        {!modelStatus?.configured ? (
          <p className="muted-copy">
            缺少：{formatMissingFields(modelStatus?.missing ?? [])}。
            <button type="button" className="link-button" onClick={onOpenSettings}>
              前往设置
            </button>
          </p>
        ) : null}
        {analysisReadyMessage ? (
          <p className="muted-copy">{analysisReadyMessage}</p>
        ) : null}
        {analysisError ? (
          <ErrorMessage
            title={analysisError.title}
            message={analysisError.message}
            details={analysisError.details}
            correlationId={analysisError.correlationId}
          />
        ) : null}
        <div className="action-row workbench-actions">
          <Button
            variant="primary"
            onClick={() => void handleAnalysisEntry()}
            disabled={isModelStatusLoading || isBatchAnalyzing || !modelStatus?.configured}
          >
            {!modelStatus?.configured
              ? "完成模型配置"
              : isBatchAnalyzing
                ? "分析中..."
                : analysisStats.pendingPages + analysisStats.pendingVisualModules > 0
                  ? `分析新内容（${analysisStats.pendingPages + analysisStats.pendingVisualModules}）`
                    : "分析新内容"}
          </Button>
          {!modelStatus?.configured ? (
            <Button onClick={onOpenSettings}>打开设置</Button>
          ) : null}
        </div>
      </section>

      {reanalysisSummary ? (
        <ReanalysisContextSummary
          summary={reanalysisSummary}
          modelReady={Boolean(modelStatus?.configured)}
          isRunning={isReanalyzing}
          onDefaultReanalysis={() => void handleDefaultReanalysis()}
          onReturn={onReturnToSource}
          onOpenSettings={onOpenSettings}
        />
      ) : null}

      <PrivacyNotice
        open={showPrivacyNotice}
        onConfirm={() => void handlePrivacyConfirm()}
        onCancel={() => {
          setShowPrivacyNotice(false);
          setPendingAction(null);
        }}
        isSubmitting={isAcceptingPrivacy}
      />
    </div>
  );
}

function ReanalysisContextSummary({
  summary,
  modelReady,
  isRunning,
  onDefaultReanalysis,
  onReturn,
  onOpenSettings,
}: {
  summary: ReanalysisSummary;
  modelReady: boolean;
  isRunning: boolean;
  onDefaultReanalysis: () => void;
  onReturn: () => void;
  onOpenSettings: () => void;
}) {
  return (
    <section className="panel panel-wide reanalysis-context-panel">
      <div className="panel-header">
        <div>
          <p className="eyebrow">重分析上下文</p>
          <h2>来自{summary.sourceTab}的选择</h2>
          <p className="muted-copy">
            已重新查询后端数据：{summary.selectedKind}，选择 {summary.selectionCount} 项，
            预计可重分析 {summary.analyzablePages} 页，已有 JSON {summary.existingJsonPages} 页，
            失败项 {summary.failedItems} 个。
          </p>
        </div>
        <StatusBadge tone={summary.failedItems > 0 ? "warning" : "success"}>
          {summary.selectionCount} 项
        </StatusBadge>
      </div>

      <div className="reanalysis-summary-grid">
        <SummaryItem label="对象类型" value={summary.selectedKind} />
        <SummaryItem label="选择数量" value={`${summary.selectionCount}`} />
        <SummaryItem label="待处理页" value={`${summary.analyzablePages}`} />
        <SummaryItem label="已有 JSON" value={`${summary.existingJsonPages}`} />
        <SummaryItem label="失败项" value={`${summary.failedItems}`} />
      </div>

      <div className="reanalysis-selected-list" aria-label="重分析选择对象">
        {summary.selectedLabels.slice(0, 6).map((label) => (
          <span key={label}>{label}</span>
        ))}
        {summary.selectedLabels.length > 6 ? (
          <span>另有 {summary.selectedLabels.length - 6} 项</span>
        ) : null}
      </div>

      <div className="action-row workbench-actions">
        <Button
          variant="primary"
          onClick={onDefaultReanalysis}
          disabled={!modelReady || isRunning || summary.analyzablePages === 0}
        >
          {isRunning
            ? "重分析中..."
            : summary.retryFailedOnly
              ? "重试失败项"
              : "默认重分析"}
        </Button>
        <Button disabled title="后端自定义提示词重分析能力尚未接入">
          自定义提示词
        </Button>
        <Button disabled title="可信 JSON 编辑/微调保存管线尚未接入">
          JSON 编辑
        </Button>
        {!modelReady ? <Button onClick={onOpenSettings}>打开设置</Button> : null}
        <Button onClick={onReturn}>返回媒体管理</Button>
      </div>
    </section>
  );
}

function SummaryItem({ label, value }: { label: string; value: string }) {
  return (
    <div className="reanalysis-summary-item">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function buildReanalysisSummary(
  context: NavigationContext,
  documents: DocumentDto[],
  pagesByDocument: Record<string, PageWorkbenchDto[]>,
): ReanalysisSummary {
  const labels: string[] = [];
  let analyzablePages = 0;
  let existingJsonPages = 0;
  let failedItems = 0;

  if (context.selected_kind === "document" || context.selected_kind === "document_batch") {
    for (const documentId of context.selected_ids) {
      const doc = documents.find((item) => item.document_id === documentId);
      const pages = pagesByDocument[documentId] ?? [];
      labels.push(doc?.original_filename ?? documentId);
      analyzablePages += pages.filter((page) => canAnalyzePage(page) || hasVisualModules(page)).length;
      existingJsonPages += pages.filter((page) => page.analysis_summary !== null).length;
      failedItems += doc?.status === "failed" ? 1 : 0;
      failedItems += pages.filter((page) => page.status === "failed").length;
      failedItems += pages.reduce(
        (sum, page) => sum + countValue(page.failed_visual_module_count),
        0,
      );
    }
  } else {
    const pageMap = new Map<string, { page: PageWorkbenchDto; doc?: DocumentDto }>();
    for (const doc of documents) {
      for (const page of pagesByDocument[doc.document_id] ?? []) {
        pageMap.set(page.page_id, { page, doc });
      }
    }
    for (const pageId of context.selected_ids) {
      const match = pageMap.get(pageId);
      labels.push(
        match
          ? `${match.doc?.original_filename ?? match.page.document_id} 第 ${match.page.page_number} 页`
          : pageId,
      );
      if (match?.page && canAnalyzePage(match.page)) {
        analyzablePages += 1;
      }
      if (match?.page.analysis_summary) {
        existingJsonPages += 1;
      }
      if (!match || match.page.status === "failed") {
        failedItems += 1;
      }
    }
  }

  return {
    selectedKind: selectedKindLabel(context.selected_kind),
    selectionCount: context.selection_count,
    sourceTab: context.source_tab === "mediaManagement" ? "媒体管理" : context.source_tab,
    selectedLabels: labels,
    analyzablePages,
    existingJsonPages,
    failedItems,
    retryFailedOnly: context.retry_failed_only,
  };
}

function canAnalyzePage(page: PageWorkbenchDto) {
  return (
    Boolean(page.image_path) &&
    (page.status === "rendered" || page.status === "failed" || page.status === "analyzed")
  );
}

function hasVisualModules(page: PageWorkbenchDto) {
  return countValue(page.visual_module_count) > 0;
}

function findWorkbenchPage(
  pageId: string,
  pagesByDocument: Record<string, PageWorkbenchDto[]>,
) {
  for (const pages of Object.values(pagesByDocument)) {
    const page = pages.find((item) => item.page_id === pageId);
    if (page) return page;
  }
  return undefined;
}

function selectedKindLabel(kind: NavigationContext["selected_kind"]) {
  switch (kind) {
    case "document":
      return "单个媒体";
    case "document_batch":
      return "批量媒体";
    case "page":
      return "单页";
    case "page_batch":
      return "批量页面";
    default:
      return kind;
  }
}

function formatMissingFields(missing: string[]): string {
  const labels: Record<string, string> = {
    model_provider: "Provider",
    model_name: "Model Name",
    endpoint: "Base URL 或自定义 Endpoint",
    api_key: "API Key",
  };
  return missing.map((key) => labels[key] ?? key).join("、") || "模型配置";
}

export function computeAnalysisStats(
  documents: DocumentDto[],
  pagesByDocument: Record<string, PageWorkbenchDto[]>,
) {
  let pendingPages = 0;
  let analyzedPages = 0;
  let failedPages = 0;
  let documentsWithPending = 0;
  let totalVisualModules = 0;
  let pendingVisualModules = 0;
  let succeededVisualModules = 0;
  let failedVisualModules = 0;
  let documentsWithPendingVisualModules = 0;
  const allPages: PageWorkbenchDto[] = [];

  for (const doc of documents) {
    const pages = pagesByDocument[doc.document_id] ?? [];
    allPages.push(...pages);
    let docPending = 0;
    let docPendingVisualModules = 0;
    for (const page of pages) {
      if (hasVisualModuleCounts(page)) {
        totalVisualModules += countValue(page.visual_module_count);
        pendingVisualModules += countValue(page.pending_visual_module_count);
        succeededVisualModules += countValue(page.succeeded_visual_module_count);
        failedVisualModules += countValue(page.failed_visual_module_count);
        docPendingVisualModules += countValue(page.pending_visual_module_count);
      } else if (page.status === "rendered") {
        pendingPages += 1;
        docPending += 1;
      } else if (page.status === "analyzed") {
        analyzedPages += 1;
      } else if (page.status === "failed") {
        failedPages += 1;
      }
    }
    if (docPending > 0) {
      documentsWithPending += 1;
    }
    if (docPendingVisualModules > 0) {
      documentsWithPendingVisualModules += 1;
    }
  }

  const hasLegacyPageStats = allPages.some(
    (page) => !hasVisualModuleCounts(page),
  );
  const hasVisualModuleStats = allPages.some(hasVisualModuleCounts);

  return {
    pendingPages,
    analyzedPages,
    failedPages,
    documentsWithPending,
    totalVisualModules,
    pendingVisualModules,
    succeededVisualModules,
    failedVisualModules,
    documentsWithPendingVisualModules,
    hasLegacyPageStats,
    hasVisualModuleStats,
  };
}

function hasVisualModuleCounts(page: PageWorkbenchDto): boolean {
  return [
    page.visual_module_count,
    page.pending_visual_module_count,
    page.succeeded_visual_module_count,
    page.failed_visual_module_count,
  ].every((value) => typeof value === "number" && Number.isFinite(value));
}

function countValue(value: number | undefined): number {
  return typeof value === "number" && Number.isFinite(value)
    ? Math.max(0, value)
    : 0;
}

export function formatAnalysisOverview(
  stats: ReturnType<typeof computeAnalysisStats>,
): string {
  const pageSummary = () => {
    const pendingDocuments =
      stats.documentsWithPending > 0
        ? `（${stats.documentsWithPending} 个文档仍有待分析页面）`
        : "";
    const failed = stats.failedPages > 0 ? `，失败 ${stats.failedPages} 页` : "";
    return `配置模型后可批量分析新页面；待分析 ${stats.pendingPages} 页${pendingDocuments}，已分析 ${stats.analyzedPages} 页${failed}。`;
  };

  const visualSummary = () => {
    const pendingDocuments =
      stats.documentsWithPendingVisualModules > 0
        ? `（${stats.documentsWithPendingVisualModules} 个文档仍有待分析模块）`
        : "";
    const failed =
      stats.failedVisualModules > 0
        ? `，失败 ${stats.failedVisualModules} 个`
        : "";
    return `视觉模块共 ${stats.totalVisualModules} 个，待分析 ${stats.pendingVisualModules} 个${pendingDocuments}，已完成 ${stats.succeededVisualModules} 个${failed}。`;
  };

  if (stats.hasLegacyPageStats && stats.hasVisualModuleStats) {
    return `${pageSummary()} ${visualSummary()}`;
  }
  return stats.hasVisualModuleStats ? visualSummary() : pageSummary();
}

export function formatBatchMessage(
  prefix: string,
  result: AnalysisBatchResultDto,
) {
  const summaries: string[] = [];
  if (result.total_pages > 0) {
    summaries.push(
      `页面共 ${result.total_pages} 页，成功 ${result.succeeded_pages} 页，失败 ${result.failed_pages} 页，跳过 ${result.skipped_pages} 页`,
    );
  }
  if (result.total_visual_modules > 0) {
    summaries.push(
      `视觉模块共 ${result.total_visual_modules} 个，成功 ${result.succeeded_visual_modules} 个，失败 ${result.failed_visual_modules} 个，跳过 ${result.skipped_visual_modules} 个`,
    );
  }
  return `${prefix}：${summaries.join("；") || "没有需要处理的内容"}。`;
}

export function formatCombinedBatchMessage(prefix: string, results: AnalysisBatchResultDto[]) {
  const totals = results.reduce(
    (sum, result) => ({
      total_pages: sum.total_pages + result.total_pages,
      succeeded_pages: sum.succeeded_pages + result.succeeded_pages,
      failed_pages: sum.failed_pages + result.failed_pages,
      skipped_pages: sum.skipped_pages + result.skipped_pages,
      total_visual_modules: sum.total_visual_modules + result.total_visual_modules,
      succeeded_visual_modules:
        sum.succeeded_visual_modules + result.succeeded_visual_modules,
      failed_visual_modules:
        sum.failed_visual_modules + result.failed_visual_modules,
      skipped_visual_modules:
        sum.skipped_visual_modules + result.skipped_visual_modules,
    }),
    {
      total_pages: 0,
      succeeded_pages: 0,
      failed_pages: 0,
      skipped_pages: 0,
      total_visual_modules: 0,
      succeeded_visual_modules: 0,
      failed_visual_modules: 0,
      skipped_visual_modules: 0,
    },
  );
  return formatBatchMessage(prefix, {
    job_id: "combined",
    ...totals,
    status: results.some((result) => result.status.includes("failed"))
      ? "succeeded_with_failures"
      : "succeeded",
    error: results.find((result) => result.error)?.error ?? null,
    updated_at: results[results.length - 1]?.updated_at ?? "",
  });
}

export function getBatchResultError(
  result: AnalysisBatchResultDto,
): AnalysisDisplayError | null {
  return result.error
    ? toDisplayError(
        result.error,
        result.status === "succeeded_with_failures" ? "分析部分完成" : "分析失败",
      )
    : null;
}

function formatPageAnalysisMessage(prefix: string, results: AnalysisResultDto[]) {
  const succeeded = results.filter((result) => result.status === "succeeded").length;
  const failed = results.length - succeeded;
  return `${prefix}：共 ${results.length} 页，成功 ${succeeded} 页，失败 ${failed} 页。`;
}

function toDisplayError(error: AppErrorDto, title: string): AnalysisDisplayError {
  return {
    title,
    message: error.message,
    details: error.details,
    correlationId: error.correlation_id,
  };
}

function extractError(error: unknown): AnalysisDisplayError {
  if (typeof error === "object" && error !== null) {
    const e = error as Record<string, unknown>;
    const msg = typeof e.message === "string" ? e.message : null;
    const details = typeof e.details === "string" ? e.details : null;
    const cid = typeof e.correlation_id === "string" ? e.correlation_id : null;
    if (msg) return { title: "分析失败", message: msg, details, correlationId: cid };
  }
  if (error instanceof Error) return { title: "分析失败", message: error.message };
  if (typeof error === "string") return { title: "分析失败", message: error };
  return { title: "分析失败", message: "任务命令调用失败，请稍后重试。" };
}
