import { useCallback, useEffect, useState } from "react";
import { Button } from "../../../components/common/Button";
import { StatusBadge } from "../../../components/common/StatusBadge";
import { tauriClient } from "../../../lib/tauriClient";
import type { IndexStatusDto } from "../../../types/app";

interface IndexStatusPanelProps {
  workspaceReady: boolean;
  isActive: boolean;
}

export function IndexStatusPanel({ workspaceReady, isActive }: IndexStatusPanelProps) {
  const [indexStatus, setIndexStatus] = useState<IndexStatusDto | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isRebuilding, setIsRebuilding] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!workspaceReady) {
      setIndexStatus(null);
      return;
    }
    setIsLoading(true);
    setError(null);
    try {
      setIndexStatus(await tauriClient.getIndexStatus());
    } catch (err) {
      setError(extractMessage(err));
    } finally {
      setIsLoading(false);
    }
  }, [workspaceReady]);

  useEffect(() => {
    if (!workspaceReady || !isActive) {
      return;
    }
    void refresh();
  }, [workspaceReady, isActive, refresh]);

  useEffect(() => {
    if (!workspaceReady || !isActive || indexStatus?.status !== "building") {
      return;
    }
    const timer = window.setInterval(() => void refresh(), 2000);
    return () => window.clearInterval(timer);
  }, [workspaceReady, isActive, indexStatus?.status, refresh]);

  async function handleRebuild() {
    setIsRebuilding(true);
    setError(null);
    try {
      await tauriClient.startIndexRebuild();
      await refresh();
    } catch (err) {
      setError(extractMessage(err));
    } finally {
      setIsRebuilding(false);
    }
  }

  if (!workspaceReady) {
    return null;
  }

  const label = isLoading
    ? "检查中"
    : indexStatus?.status === "ready"
      ? "索引可用"
      : indexStatus?.status === "building"
        ? "构建中"
        : indexStatus?.status === "failed"
          ? "索引失败"
          : indexStatus?.status === "needs_rebuild"
            ? "需要重建"
            : "索引未建立";

  return (
    <section className="panel panel-wide">
      <div className="panel-header">
        <div>
          <p className="eyebrow">检索索引</p>
          <h2>BM25 索引</h2>
          <p className="muted-copy">
            已索引 {indexStatus?.indexed_page_count ?? 0} 页，可索引{" "}
            {indexStatus?.analyzable_page_count ?? 0} 页
            {(indexStatus?.pending_index_page_count ?? 0) > 0
              ? `（${indexStatus?.pending_index_page_count} 页待纳入）`
              : ""}
            。
            {indexStatus?.stale_reason ? ` ${indexStatus.stale_reason}` : ""}
          </p>
        </div>
        <StatusBadge
          tone={
            indexStatus?.status === "ready"
              ? "success"
              : indexStatus?.status === "building"
                ? "warning"
                : indexStatus?.status === "failed"
                  ? "danger"
                  : "neutral"
          }
        >
          {label}
        </StatusBadge>
      </div>
      {error ? <p className="job-error">{error}</p> : null}
      {indexStatus?.error_summary ? (
        <p className="job-error">{indexStatus.error_summary}</p>
      ) : null}
      {(indexStatus?.analyzable_page_count ?? 0) === 0 ? (
        <p className="muted-copy">
          索引基于已完成的页面分析结果。请先在「模型分析」中配置模型并分析页面；若导入失败，请先重试
          PDF 导入。
        </p>
      ) : null}
      <div className="action-row workbench-actions">
        <Button
          variant="primary"
          onClick={() => void handleRebuild()}
          disabled={
            isRebuilding ||
            indexStatus?.status === "building" ||
            indexStatus?.can_rebuild === false ||
            (indexStatus?.analyzable_page_count ?? 0) === 0
          }
        >
          {isRebuilding || indexStatus?.status === "building"
            ? "索引重建中..."
            : indexStatus?.status === "not_built"
              ? "构建索引"
              : "重建索引"}
        </Button>
      </div>
    </section>
  );
}

function extractMessage(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as Record<string, unknown>).message);
  }
  return typeof error === "string" ? error : "索引操作失败";
}
