import { useEffect, useMemo, useRef, useState } from "react";
import { Button } from "../../components/common/Button";
import { EmptyState } from "../../components/common/EmptyState";
import { ErrorMessage } from "../../components/common/ErrorMessage";
import { StatusBadge } from "../../components/common/StatusBadge";
import { tauriClient } from "../../lib/tauriClient";
import type {
  DocumentDto,
  IndexStatusDto,
  JobDto,
  PageWorkbenchDto,
  WorkspaceStatusDto,
} from "../../types/app";
import { AnalysisJobList } from "./components/AnalysisJobList";
import { JobList } from "./components/JobList";
import { WorkspacePicker } from "./components/WorkspacePicker";

interface WorkbenchPageProps {
  workspaceStatus: WorkspaceStatusDto;
  isWorkspaceLoading: boolean;
  isActive: boolean;
  onChooseWorkspace: () => void;
  onOpenSettings: () => void;
  onOpenMediaImport: () => void;
  onOpenMediaManagement: () => void;
  onOpenAnalysis: () => void;
  onOpenExport: () => void;
  onOpenIndex: () => void;
  onOpenSearch: () => void;
}

export function WorkbenchPage({
  workspaceStatus,
  isWorkspaceLoading,
  isActive,
  onChooseWorkspace,
  onOpenSettings,
  onOpenMediaImport,
  onOpenMediaManagement,
  onOpenAnalysis,
  onOpenExport,
  onOpenIndex,
  onOpenSearch,
}: WorkbenchPageProps) {
  const workspaceReady = workspaceStatus.status === "ready";
  const workspaceKey = workspaceStatus.workspace_path ?? "current";
  const recoveredWorkspaceRef = useRef<string | null>(null);
  const jobsGenRef = useRef(0);
  const docsGenRef = useRef(0);
  const indexGenRef = useRef(0);
  const [jobs, setJobs] = useState<JobDto[]>([]);
  const [documents, setDocuments] = useState<DocumentDto[]>([]);
  const [pagesByDocument, setPagesByDocument] = useState<Record<string, PageWorkbenchDto[]>>({});
  const [indexStatus, setIndexStatus] = useState<IndexStatusDto | null>(null);
  const [isJobsLoading, setIsJobsLoading] = useState(false);
  const [isDocsLoading, setIsDocsLoading] = useState(false);
  const [isIndexLoading, setIsIndexLoading] = useState(false);
  const [error, setError] = useState<{ message: string; correlationId?: string | null } | null>(null);

  async function refreshJobs(options: { recoverInterrupted?: boolean } = {}) {
    if (!workspaceReady) {
      setJobs([]);
      return;
    }

    const gen = ++jobsGenRef.current;
    setIsJobsLoading(true);
    try {
      if (options.recoverInterrupted) {
        recoveredWorkspaceRef.current = workspaceKey;
        await tauriClient.recoverInterruptedJobs();
        await tauriClient.recoverInterruptedAnalysisPages();
      }
      const result = await tauriClient.listJobs();
      if (gen === jobsGenRef.current) {
        setJobs(result);
      }
    } catch (err) {
      if (gen === jobsGenRef.current) {
        setError(extractError(err));
      }
    } finally {
      if (gen === jobsGenRef.current) {
        setIsJobsLoading(false);
      }
    }
  }

  async function refreshDocuments() {
    if (!workspaceReady) {
      setDocuments([]);
      setPagesByDocument({});
      return;
    }

    const gen = ++docsGenRef.current;
    setIsDocsLoading(true);
    try {
      const docs = await tauriClient.listDocuments();
      if (gen !== docsGenRef.current) {
        return;
      }
      const pagesMap: Record<string, PageWorkbenchDto[]> = {};
      for (const doc of docs) {
        try {
          pagesMap[doc.document_id] = await tauriClient.listWorkbenchPages(doc.document_id);
        } catch {
          pagesMap[doc.document_id] = [];
        }
        if (gen !== docsGenRef.current) {
          return;
        }
      }
      setDocuments(docs);
      setPagesByDocument(pagesMap);
    } catch (err) {
      if (gen === docsGenRef.current) {
        setDocuments([]);
        setPagesByDocument({});
        setError(extractError(err));
      }
    } finally {
      if (gen === docsGenRef.current) {
        setIsDocsLoading(false);
      }
    }
  }

  async function refreshIndexStatus() {
    if (!workspaceReady) {
      setIndexStatus(null);
      return;
    }

    const gen = ++indexGenRef.current;
    setIsIndexLoading(true);
    try {
      const result = await tauriClient.getIndexStatus();
      if (gen === indexGenRef.current) {
        setIndexStatus(result);
      }
    } catch (err) {
      if (gen === indexGenRef.current) {
        setIndexStatus(null);
        setError(extractError(err));
      }
    } finally {
      if (gen === indexGenRef.current) {
        setIsIndexLoading(false);
      }
    }
  }

  async function refreshAll(options: { recoverInterrupted?: boolean } = {}) {
    setError(null);
    await Promise.all([
      refreshJobs(options),
      refreshDocuments(),
      refreshIndexStatus(),
    ]);
  }

  useEffect(() => {
    ++jobsGenRef.current;
    ++docsGenRef.current;
    ++indexGenRef.current;
    setError(null);
    if (!workspaceReady) {
      setJobs([]);
      setDocuments([]);
      setPagesByDocument({});
      setIndexStatus(null);
      return;
    }

    void refreshAll({
      recoverInterrupted: recoveredWorkspaceRef.current !== workspaceKey,
    });
  }, [workspaceReady, workspaceKey]);

  useEffect(() => {
    if (!workspaceReady || !isActive) {
      return;
    }
    void refreshAll();
  }, [workspaceReady, isActive]);

  const workbenchStats = useMemo(
    () => computeWorkbenchStats(documents, pagesByDocument, jobs, indexStatus),
    [documents, pagesByDocument, jobs, indexStatus],
  );
  const recentJobs = useMemo(() => jobs.slice(0, 8), [jobs]);

  return (
    <div className="page-grid workbench-page workbench-overview-page">
      <section className="panel panel-wide workbench-overview-panel">
        <div className="panel-header">
          <div>
            <p className="eyebrow">工作台</p>
            <h2>工作区概览与功能分流</h2>
            <p className="muted-copy">
              工作台只展示本地账本摘要和快捷入口；导入、管理、分析、索引、搜索和导出在各自页面执行。
            </p>
          </div>
          <StatusBadge tone={workspaceReady ? "success" : "warning"}>
            {workspaceReady ? "工作区可用" : "尚未选择工作区"}
          </StatusBadge>
        </div>

        <div className="workbench-empty-block">
          <WorkspacePicker
            status={workspaceStatus}
            isLoading={isWorkspaceLoading}
            onChooseWorkspace={onChooseWorkspace}
          />
        </div>

        {error ? (
          <ErrorMessage
            title="工作台概览"
            message={error.message}
            correlationId={error.correlationId}
          />
        ) : null}

        {!workspaceReady ? (
          <EmptyState
            title="等待工作区"
            description="工作区可用后，这里会显示媒体、任务和索引摘要。"
          />
        ) : null}
      </section>

      {workspaceReady ? (
        <>
          <section className="panel panel-wide workbench-summary-panel" aria-label="工作台摘要">
            <div className="workbench-summary-grid">
              <WorkbenchMetric label="媒体" value={workbenchStats.documentCount} helper="已进入工作区" />
              <WorkbenchMetric label="页面图片" value={workbenchStats.generatedPages} helper={`共 ${workbenchStats.totalPages} 页`} />
              <WorkbenchMetric label="待分析" value={workbenchStats.pendingPages} helper="进入模型分析处理" />
              <WorkbenchMetric
                label="失败"
                value={workbenchStats.failureCount}
                helper="在媒体管理查看"
                tone={workbenchStats.failureCount > 0 ? "danger" : "neutral"}
              />
              <WorkbenchMetric label="索引页" value={workbenchStats.indexedPages} helper={workbenchStats.indexHelper} />
              <WorkbenchMetric label="处理中" value={workbenchStats.runningJobs} helper="最近任务" />
            </div>
          </section>

          <section className="panel panel-wide workbench-routing-panel">
            <div className="panel-header">
              <div>
                <p className="eyebrow">快捷入口</p>
                <h2>前往对应功能页</h2>
                <p className="muted-copy">
                  这些按钮只切换 tab，不打开文件选择器，也不直接执行业务命令。
                </p>
              </div>
              <Button onClick={() => void refreshAll()} disabled={isDocsLoading || isJobsLoading || isIndexLoading}>
                {isDocsLoading || isJobsLoading || isIndexLoading ? "刷新中" : "刷新摘要"}
              </Button>
            </div>

            <div className="workbench-route-grid">
              <RouteButton title="媒体导入" description="提交图片、PDF 与 Office 文档" onClick={onOpenMediaImport} />
              <RouteButton title="媒体管理" description="查看列表、筛选、删除与选择重分析" onClick={onOpenMediaManagement} />
              <RouteButton title="模型分析" description="分析新页面或处理重分析上下文" onClick={onOpenAnalysis} />
              <RouteButton title="BM25 索引" description="查看索引状态并进入重建页" onClick={onOpenIndex} />
              <RouteButton title="搜索" description="查询页面级内容与只读 JSON" onClick={onOpenSearch} />
              <RouteButton title="一键导出" description="导出 Markdown 与媒体包" onClick={onOpenExport} />
              <RouteButton title="设置" description="配置工作区、模型和本地服务" onClick={onOpenSettings} />
            </div>
          </section>

          <AnalysisJobList jobs={recentJobs} />

          <JobList
            jobs={recentJobs}
            isLoading={isJobsLoading}
            errorMessage={error}
            onRefresh={() => void refreshJobs()}
          />
        </>
      ) : null}
    </div>
  );
}

function RouteButton({
  title,
  description,
  onClick,
}: {
  title: string;
  description: string;
  onClick: () => void;
}) {
  return (
    <button className="workbench-route-button" type="button" onClick={onClick}>
      <strong>{title}</strong>
      <span>{description}</span>
    </button>
  );
}

function WorkbenchMetric({
  label,
  value,
  helper,
  tone = "neutral",
}: {
  label: string;
  value: number;
  helper: string;
  tone?: "neutral" | "danger";
}) {
  return (
    <div className="workbench-metric" data-tone={tone}>
      <span className="workbench-metric-label">{label}</span>
      <strong className="workbench-metric-value">{value}</strong>
      <span className="workbench-metric-helper">{helper}</span>
    </div>
  );
}

function computeWorkbenchStats(
  documents: DocumentDto[],
  pagesByDocument: Record<string, PageWorkbenchDto[]>,
  jobs: JobDto[],
  indexStatus: IndexStatusDto | null,
) {
  let totalPages = 0;
  let generatedPages = 0;
  let pendingPages = 0;
  let failedPages = 0;

  for (const doc of documents) {
    const pages = pagesByDocument[doc.document_id] ?? [];
    totalPages += doc.page_count ?? pages.length;
    for (const page of pages) {
      if (page.image_path) {
        generatedPages += 1;
      }
      if (page.status === "rendered") {
        pendingPages += 1;
      }
      if (page.status === "failed") {
        failedPages += 1;
      }
    }
  }

  const failedDocuments = documents.filter((doc) => doc.status === "failed").length;
  const runningJobs = jobs.filter(
    (job) => job.status === "queued" || job.status === "running",
  ).length;
  const indexedPages = indexStatus?.indexed_page_count ?? 0;
  const indexHelper = indexStatus
    ? indexStatus.stale
      ? "索引需刷新"
      : "索引可用"
    : "未读取";

  return {
    documentCount: documents.length,
    totalPages,
    generatedPages,
    pendingPages,
    failureCount: failedDocuments + failedPages,
    runningJobs,
    indexedPages,
    indexHelper,
  };
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
  return { message: "工作台摘要读取失败，请稍后重试。" };
}
