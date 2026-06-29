import { useState } from "react";
import { Button } from "../../components/common/Button";
import { ErrorMessage } from "../../components/common/ErrorMessage";
import { tauriClient } from "../../lib/tauriClient";
import type { MediaExportResultDto } from "../../types/app";

interface ExportPageProps {
  workspaceReady: boolean;
}

export function ExportPage({ workspaceReady }: ExportPageProps) {
  const [isExporting, setIsExporting] = useState(false);
  const [exportError, setExportError] = useState<{ message: string; correlationId?: string | null } | null>(null);
  const [exportResult, setExportResult] = useState<MediaExportResultDto | null>(null);

  async function handleExport() {
    setExportError(null);
    setExportResult(null);
    try {
      const folder = await tauriClient.openExportFolderDialog();
      if (!folder) return;
      setIsExporting(true);
      const result = await tauriClient.exportMedia(folder);
      setExportResult(result);
    } catch (error) {
      setExportError(extractError(error));
    } finally {
      setIsExporting(false);
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
            <p className="eyebrow">数据导出</p>
            <h2>一键导出</h2>
            <p className="muted-copy">
              将所有已分析页面导出为 Markdown 文件和媒体资源包，可直接用于 Obsidian 等工具。
            </p>
          </div>
        </div>
        {exportError ? (
          <ErrorMessage
            title="导出失败"
            message={exportError.message}
            correlationId={exportError.correlationId}
          />
        ) : null}
        {exportResult ? (
          <p className="muted-copy">
            导出完成：{exportResult.document_count} 个文档，{exportResult.media_count} 个媒体文件。
            Markdown 文件：{exportResult.markdown_path}
          </p>
        ) : null}
        <div className="action-row workbench-actions">
          <Button
            variant="primary"
            onClick={() => void handleExport()}
            disabled={isExporting}
          >
            {isExporting ? "导出中..." : "一键导出"}
          </Button>
        </div>
      </section>
    </div>
  );
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
