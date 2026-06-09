import { useEffect, useMemo, useRef, useState } from "react";
import { Button } from "../../components/common/Button";
import { EmptyState } from "../../components/common/EmptyState";
import { ErrorMessage } from "../../components/common/ErrorMessage";
import { StatusBadge } from "../../components/common/StatusBadge";
import { tauriClient } from "../../lib/tauriClient";
import type {
  DocumentDto,
  ImportResultDto,
  JobDto,
  PageWorkbenchDto,
  WorkspaceStatusDto,
} from "../../types/app";
import { DocumentList } from "../workbench/components/DocumentList";
import { ImportResultList } from "../workbench/components/ImportResultList";

interface ImageImportPageProps {
  workspaceStatus: WorkspaceStatusDto;
  isWorkspaceLoading: boolean;
  isActive: boolean;
  onChooseWorkspace: () => void;
}

const imageTypes = new Set(["png", "jpg", "jpeg"]);

export function ImageImportPage({
  workspaceStatus,
  isWorkspaceLoading,
  isActive,
  onChooseWorkspace,
}: ImageImportPageProps) {
  const workspaceReady = workspaceStatus.status === "ready";
  const workspaceKey = workspaceStatus.workspace_path ?? "current";
  const jobsGenRef = useRef(0);
  const docsGenRef = useRef(0);
  const [jobs, setJobs] = useState<JobDto[]>([]);
  const [documents, setDocuments] = useState<DocumentDto[]>([]);
  const [pagesByDocument, setPagesByDocument] = useState<Record<string, PageWorkbenchDto[]>>({});
  const [isJobsLoading, setIsJobsLoading] = useState(false);
  const [isDocsLoading, setIsDocsLoading] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [deletingDocumentId, setDeletingDocumentId] = useState<string | null>(null);
  const [importResults, setImportResults] = useState<ImportResultDto[]>([]);
  const [error, setError] = useState<{ message: string; correlationId?: string | null } | null>(null);

  const imageDocuments = useMemo(
    () => documents.filter((doc) => imageTypes.has(doc.file_type.toLowerCase())),
    [documents],
  );

  const imagePagesByDocument = useMemo(() => {
    const next: Record<string, PageWorkbenchDto[]> = {};
    for (const doc of imageDocuments) {
      next[doc.document_id] = pagesByDocument[doc.document_id] ?? [];
    }
    return next;
  }, [imageDocuments, pagesByDocument]);

  async function refreshJobs() {
    if (!workspaceReady) {
      setJobs([]);
      return;
    }
    const gen = ++jobsGenRef.current;
    setIsJobsLoading(true);
    try {
      const result = await tauriClient.listJobs();
      if (gen === jobsGenRef.current) setJobs(result);
    } catch (err) {
      if (gen === jobsGenRef.current) setError(extractError(err));
    } finally {
      if (gen === jobsGenRef.current) setIsJobsLoading(false);
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
      if (gen !== docsGenRef.current) return;
      const pagesMap: Record<string, PageWorkbenchDto[]> = {};
      for (const doc of docs) {
        if (!imageTypes.has(doc.file_type.toLowerCase())) continue;
        try {
          pagesMap[doc.document_id] = await tauriClient.listWorkbenchPages(doc.document_id);
        } catch {
          pagesMap[doc.document_id] = [];
        }
        if (gen !== docsGenRef.current) return;
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
      if (gen === docsGenRef.current) setIsDocsLoading(false);
    }
  }

  async function handleImportImages() {
    const selected = await tauriClient.openImageImportDialog();
    if (!selected) return;
    const filePaths = Array.isArray(selected) ? selected : [selected];
    if (filePaths.length === 0) return;

    setIsImporting(true);
    setError(null);
    setImportResults([]);

    const results = await tauriClient.importMultipleImages(filePaths);
    setImportResults(results);

    await Promise.all([refreshDocuments(), refreshJobs()]);
    setIsImporting(false);
  }

  async function handleRetryImport(documentId: string) {
    setError(null);
    try {
      await tauriClient.retryImport(documentId);
      await Promise.all([refreshDocuments(), refreshJobs()]);
    } catch (err) {
      setError(extractError(err));
    }
  }

  async function handleOpenSourceFile(path: string) {
    try {
      await tauriClient.revealDocumentInFolder(path);
    } catch (err) {
      setError(extractError(err));
    }
  }

  async function handleOpenDocumentImage(page: PageWorkbenchDto) {
    const imagePath = resolveWorkspacePath(page.image_path, workspaceStatus.workspace_path);
    if (!imagePath) {
      setError({ message: "页面图片不可用，可能尚未生成或路径无效。" });
      return;
    }

    try {
      await tauriClient.revealDocumentInFolder(imagePath);
    } catch (err) {
      setError(extractError(err));
    }
  }

  async function handleDeleteDocument(documentId: string) {
    setDeletingDocumentId(documentId);
    setError(null);
    try {
      await tauriClient.deleteDocument(documentId);
      await Promise.all([refreshDocuments(), refreshJobs()]);
    } catch (err) {
      setError(extractError(err));
      await Promise.all([refreshDocuments(), refreshJobs()]);
    } finally {
      setDeletingDocumentId(null);
    }
  }

  useEffect(() => {
    ++jobsGenRef.current;
    ++docsGenRef.current;
    setError(null);
    setImportResults([]);
    setDeletingDocumentId(null);
    setIsImporting(false);
    if (!workspaceReady) {
      setJobs([]);
      setDocuments([]);
      setPagesByDocument({});
      return;
    }
    void refreshJobs();
    void refreshDocuments();
  }, [workspaceReady, workspaceKey]);

  useEffect(() => {
    if (!workspaceReady || !isActive) return;
    void refreshJobs();
    void refreshDocuments();
  }, [workspaceReady, isActive]);

  return (
    <div className="page-grid">
      <section className="panel panel-wide">
        <div className="panel-header">
          <div>
            <p className="eyebrow">图片导入</p>
            <h2>管理图片文档</h2>
            <p className="muted-copy">
              批量导入 PNG、JPG、JPEG 图片，每张图片会作为一个单页文档进入后续分析与导出流程。
            </p>
          </div>
          <StatusBadge tone={workspaceReady ? "success" : "warning"}>
            {workspaceReady ? "工作区可用" : "尚未选择工作区"}
          </StatusBadge>
        </div>

        {!workspaceReady ? (
          <div className="action-row workbench-actions">
            <Button
              variant="primary"
              onClick={onChooseWorkspace}
              disabled={isWorkspaceLoading}
            >
              {isWorkspaceLoading ? "检查中..." : "选择工作区"}
            </Button>
          </div>
        ) : null}

        {error ? (
          <ErrorMessage
            title="图片导入"
            message={error.message}
            correlationId={error.correlationId}
          />
        ) : null}

        {workspaceReady ? (
          <>
            <ImportResultList results={importResults} />
            <div className="action-row workbench-actions">
              <Button
                variant="primary"
                onClick={() => void handleImportImages()}
                disabled={isImporting}
              >
                {isImporting ? "导入中..." : "选择图片"}
              </Button>
              <Button
                onClick={() => void Promise.all([refreshDocuments(), refreshJobs()])}
                disabled={isDocsLoading || isJobsLoading || isImporting}
              >
                刷新
              </Button>
            </div>
          </>
        ) : null}
      </section>

      {workspaceReady && imageDocuments.length === 0 && !isDocsLoading ? (
        <EmptyState
          title="还没有图片文档"
          description="选择图片后，它们会以单页文档形式出现在这里。"
        />
      ) : null}

      {workspaceReady ? (
        <DocumentList
          documents={imageDocuments}
          pagesByDocument={imagePagesByDocument}
          jobs={jobs}
          isLoading={isDocsLoading}
          onRetry={(id) => void handleRetryImport(id)}
          onOpenSourceFile={(path) => void handleOpenSourceFile(path)}
          onOpenDocumentImage={(page) => void handleOpenDocumentImage(page)}
          onDeleteDocument={(documentId) => void handleDeleteDocument(documentId)}
          deletingDocumentId={deletingDocumentId}
        />
      ) : null}
    </div>
  );
}

function resolveWorkspacePath(
  relativePath: string | null | undefined,
  workspacePath: string | null | undefined,
) {
  if (!relativePath || !workspacePath) {
    return null;
  }

  const normalized = relativePath.replace(/\\/g, "/");
  if (
    normalized.startsWith("/") ||
    /^[A-Za-z]:\//.test(normalized) ||
    normalized.split("/").includes("..")
  ) {
    return null;
  }

  const separator = workspacePath.includes("\\") ? "\\" : "/";
  const root = workspacePath.replace(/[\\/]+$/, "");
  return `${root}${separator}${normalized.replace(/\//g, separator)}`;
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
