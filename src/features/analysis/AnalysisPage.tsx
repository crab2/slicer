import { useEffect, useState } from "react";
import { Button } from "../../components/common/Button";
import { ErrorMessage } from "../../components/common/ErrorMessage";
import { StatusBadge } from "../../components/common/StatusBadge";
import { PrivacyNotice } from "../settings/components/PrivacyNotice";
import { tauriClient } from "../../lib/tauriClient";
import type {
  AnalysisBatchResultDto,
  DocumentDto,
  ModelConfigurationStatusDto,
  PageWorkbenchDto,
} from "../../types/app";

interface AnalysisPageProps {
  workspaceReady: boolean;
  isActive: boolean;
  onOpenSettings: () => void;
}

export function AnalysisPage({
  workspaceReady,
  isActive,
  onOpenSettings,
}: AnalysisPageProps) {
  const [modelStatus, setModelStatus] = useState<ModelConfigurationStatusDto | null>(null);
  const [isModelStatusLoading, setIsModelStatusLoading] = useState(false);
  const [showPrivacyNotice, setShowPrivacyNotice] = useState(false);
  const [isAcceptingPrivacy, setIsAcceptingPrivacy] = useState(false);
  const [analysisReadyMessage, setAnalysisReadyMessage] = useState<string | null>(null);
  const [analysisError, setAnalysisError] = useState<{ message: string; correlationId?: string | null } | null>(null);
  const [isBatchAnalyzing, setIsBatchAnalyzing] = useState(false);
  const [pendingBatchAction, setPendingBatchAction] = useState<"new-pages" | null>(null);
  const [documents, setDocuments] = useState<DocumentDto[]>([]);
  const [pagesByDocument, setPagesByDocument] = useState<Record<string, PageWorkbenchDto[]>>({});

  const analysisStats = computeAnalysisStats(documents, pagesByDocument);
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
    if (!workspaceReady) return;
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
    }
  }

  useEffect(() => {
    if (!workspaceReady || !isActive) return;
    void refreshModelStatus();
    void refreshDocuments();
  }, [workspaceReady, isActive]);

  useEffect(() => {
    if (!workspaceReady || !isActive || !isBatchAnalyzing) return;
    const timer = window.setInterval(() => {
      void refreshDocuments();
    }, 2000);
    return () => window.clearInterval(timer);
  }, [workspaceReady, isActive, isBatchAnalyzing]);

  async function handleAnalysisEntry() {
    setAnalysisError(null);
    setAnalysisReadyMessage(null);
    if (!modelStatus?.configured) {
      onOpenSettings();
      return;
    }
    if (modelStatus.requires_privacy_notice && !modelStatus.privacy_notice_accepted) {
      setPendingBatchAction("new-pages");
      setShowPrivacyNotice(true);
      return;
    }
    await executeAnalyzeNewPages();
  }

  async function executeAnalyzeNewPages() {
    setIsBatchAnalyzing(true);
    try {
      const result = await tauriClient.analyzeNewPages();
      setAnalysisReadyMessage(formatBatchMessage("新页面批量分析完成", result));
      await refreshDocuments();
    } catch (error) {
      setAnalysisError(extractError(error));
      await refreshDocuments();
    } finally {
      setIsBatchAnalyzing(false);
    }
  }

  async function handlePrivacyConfirm() {
    setIsAcceptingPrivacy(true);
    try {
      await tauriClient.acceptPrivacyNotice();
      setShowPrivacyNotice(false);
      const status = await tauriClient.getModelConfigurationStatus();
      setModelStatus(status);
      if (pendingBatchAction === "new-pages") {
        await executeAnalyzeNewPages();
      } else {
        setAnalysisReadyMessage("隐私提示已确认。可以开始批量分析。");
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
    <div className="page-grid">
      <section className="panel panel-wide">
        <div className="panel-header">
          <div>
            <p className="eyebrow">页面分析</p>
            <h2>模型分析</h2>
            <p className="muted-copy">
              配置模型后可批量分析新页面；待分析 {analysisStats.pendingPages} 页
              {analysisStats.documentsWithPending > 0
                ? `（${analysisStats.documentsWithPending} 个文档仍有待分析页面）`
                : ""}
              ，已分析 {analysisStats.analyzedPages} 页
              {analysisStats.failedPages > 0
                ? `，失败 ${analysisStats.failedPages} 页`
                : ""}
              。
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
            title="分析失败"
            message={analysisError.message}
            correlationId={analysisError.correlationId}
          />
        ) : null}
        <div className="action-row workbench-actions">
          <Button
            variant="primary"
            onClick={() => void handleAnalysisEntry()}
            disabled={isModelStatusLoading || isBatchAnalyzing || !analysisConfigured}
          >
            {!modelStatus?.configured
              ? "完成模型配置"
              : isBatchAnalyzing
                ? "批量分析中..."
                : analysisStats.pendingPages > 0
                  ? `分析新页面（${analysisStats.pendingPages}）`
                  : "分析新页面"}
          </Button>
          {!modelStatus?.configured ? (
            <Button onClick={onOpenSettings}>打开设置</Button>
          ) : null}
        </div>
      </section>

      <PrivacyNotice
        open={showPrivacyNotice}
        onConfirm={() => void handlePrivacyConfirm()}
        onCancel={() => {
          setShowPrivacyNotice(false);
          setPendingBatchAction(null);
        }}
        isSubmitting={isAcceptingPrivacy}
      />
    </div>
  );
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

function computeAnalysisStats(
  documents: DocumentDto[],
  pagesByDocument: Record<string, PageWorkbenchDto[]>,
) {
  let pendingPages = 0;
  let analyzedPages = 0;
  let failedPages = 0;
  let documentsWithPending = 0;

  for (const doc of documents) {
    const pages = pagesByDocument[doc.document_id] ?? [];
    let docPending = 0;
    for (const page of pages) {
      if (page.status === "rendered") {
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
  }

  return { pendingPages, analyzedPages, failedPages, documentsWithPending };
}

function formatBatchMessage(prefix: string, result: AnalysisBatchResultDto) {
  return `${prefix}：共 ${result.total_pages} 页，成功 ${result.succeeded_pages} 页，失败 ${result.failed_pages} 页，跳过 ${result.skipped_pages} 页。`;
}

function extractError(error: unknown): { message: string; correlationId?: string | null } {
  if (typeof error === "object" && error !== null) {
    const e = error as Record<string, unknown>;
    const msg = typeof e.message === "string" ? e.message : null;
    const cid = typeof e.correlation_id === "string" ? e.correlation_id : null;
    if (msg) return { message: msg, correlationId: cid };
  }
  if (error instanceof Error) return { message: error.message };
  if (typeof error === "string") return { message: error };
  return { message: "任务命令调用失败，请稍后重试。" };
}
