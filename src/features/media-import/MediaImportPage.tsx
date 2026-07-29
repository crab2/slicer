import { useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Button } from "../../components/common/Button";
import { EmptyState } from "../../components/common/EmptyState";
import { ErrorMessage } from "../../components/common/ErrorMessage";
import { StatusBadge } from "../../components/common/StatusBadge";
import {
  getUnsupportedReason,
  isSupportedFileType,
  SUPPORTED_EXTENSIONS,
} from "../../lib/fileValidation";
import { tauriClient } from "../../lib/tauriClient";
import type { ImportResultDto, WorkspaceStatusDto } from "../../types/app";
import { ImportResultList } from "../workbench/components/ImportResultList";

interface MediaImportPageProps {
  workspaceStatus: WorkspaceStatusDto;
  isWorkspaceLoading: boolean;
  isActive: boolean;
  onChooseWorkspace: () => void;
}

export function MediaImportPage({
  workspaceStatus,
  isWorkspaceLoading,
  isActive,
  onChooseWorkspace,
}: MediaImportPageProps) {
  const workspaceReady = workspaceStatus.status === "ready";
  const importLockRef = useRef(false);
  const workspaceReadyRef = useRef(workspaceReady);
  const isActiveRef = useRef(isActive);
  const importMediaFilesRef = useRef<(filePaths: unknown[]) => Promise<void>>(async () => undefined);
  const [isImporting, setIsImporting] = useState(false);
  const [isDragActive, setIsDragActive] = useState(false);
  const [importResults, setImportResults] = useState<ImportResultDto[]>([]);
  const [error, setError] = useState<{ message: string; correlationId?: string | null } | null>(null);

  workspaceReadyRef.current = workspaceReady;
  isActiveRef.current = isActive;

  const supportedLabel = useMemo(
    () => SUPPORTED_EXTENSIONS.map((ext) => ext.slice(1).toUpperCase()).join("、"),
    [],
  );

  async function handleChooseFiles() {
    if (importLockRef.current) {
      return;
    }
    const selected = await tauriClient.openMediaImportDialog();
    if (!selected) {
      return;
    }
    await importMediaFiles(Array.isArray(selected) ? selected : [selected]);
  }

  async function importMediaFiles(filePaths: unknown[]) {
    if (importLockRef.current) {
      return;
    }

    const normalizedPaths = normalizeImportPaths(filePaths);
    if (normalizedPaths.length === 0) {
      return;
    }

    const pathlessResults = filePaths
      .filter((path) => typeof path !== "string")
      .map<ImportResultDto>((path, index) => ({
        file_name: path instanceof File && path.name ? path.name : `拖拽文件 ${index + 1}`,
        status: "failed",
        error: "无法从拖拽事件读取本地路径，请使用“选择文件”导入。",
      }));

    const invalidResults = normalizedPaths
      .filter((path) => !isSupportedFileType(path))
      .map<ImportResultDto>((path) => ({
        file_name: fileNameFromPath(path),
        status: "unsupported",
        error: getUnsupportedReason(path),
      }));

    const importablePaths = normalizedPaths.filter((path) => isSupportedFileType(path));

    importLockRef.current = true;
    setIsImporting(true);
    setIsDragActive(false);
    setError(null);
    setImportResults([...pathlessResults, ...invalidResults]);

    try {
      const results =
        importablePaths.length > 0
          ? await tauriClient.importMultipleFiles(importablePaths)
          : [];
      setImportResults([...pathlessResults, ...invalidResults, ...results]);
    } catch (err) {
      setError(extractError(err));
    } finally {
      importLockRef.current = false;
      setIsImporting(false);
    }
  }

  importMediaFilesRef.current = importMediaFiles;

  function canAcceptDrop() {
    return workspaceReady && isActive && !importLockRef.current;
  }

  function handleDragEnter(event: React.DragEvent<HTMLElement>) {
    event.preventDefault();
    event.stopPropagation();
    if (canAcceptDrop()) {
      setIsDragActive(true);
    }
  }

  function handleDragOver(event: React.DragEvent<HTMLElement>) {
    event.preventDefault();
    event.stopPropagation();
    if (canAcceptDrop()) {
      event.dataTransfer.dropEffect = "copy";
      setIsDragActive(true);
    } else {
      event.dataTransfer.dropEffect = "none";
    }
  }

  function handleDragLeave(event: React.DragEvent<HTMLElement>) {
    event.preventDefault();
    event.stopPropagation();
    const nextTarget = event.relatedTarget;
    if (!(nextTarget instanceof Node) || !event.currentTarget.contains(nextTarget)) {
      setIsDragActive(false);
    }
  }

  async function handleDrop(event: React.DragEvent<HTMLElement>) {
    event.preventDefault();
    event.stopPropagation();
    setIsDragActive(false);
    if (!canAcceptDrop()) {
      if (importLockRef.current) {
        setImportResults([
          {
            file_name: "拖拽内容",
            status: "failed",
            error: "已有导入正在进行，请等待当前批次完成后再拖入新文件。",
          },
        ]);
      }
      return;
    }

    const files = Array.from(event.dataTransfer.files);
    if (files.length === 0) {
      setImportResults([
        {
          file_name: "拖拽内容",
          status: "failed",
          error: "请拖入本地文件。",
        },
      ]);
      return;
    }

    await importMediaFiles(files.map((file) => getDroppedFilePath(file) ?? file));
  }

  useEffect(() => {
    if (!workspaceReady) {
      setImportResults([]);
      setError(null);
      setIsDragActive(false);
      setIsImporting(false);
    }
  }, [workspaceReady]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    try {
      void getCurrentWindow()
        .onDragDropEvent((event) => {
          const ready =
            workspaceReadyRef.current && isActiveRef.current && !importLockRef.current;
          if (event.payload.type === "enter" || event.payload.type === "over") {
            setIsDragActive(ready);
            return;
          }
          if (event.payload.type === "leave") {
            setIsDragActive(false);
            return;
          }
          setIsDragActive(false);
          if (event.payload.type !== "drop") {
            return;
          }
          if (!ready) {
            return;
          }
          const paths = Array.isArray(event.payload.paths) ? event.payload.paths : [];
          if (paths.length > 0) {
            void importMediaFilesRef.current(paths);
          }
        })
        .then((nextUnlisten) => {
          if (cancelled) {
            nextUnlisten();
          } else {
            unlisten = nextUnlisten;
          }
        })
        .catch(() => {
          unlisten = null;
        });
    } catch {
      unlisten = null;
    }
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return (
    <div
      className="page-grid media-import-page"
      data-drag-active={isDragActive}
      onDragEnter={handleDragEnter}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={(event) => void handleDrop(event)}
    >
      <section className="panel panel-wide media-import-panel">
        <div className="panel-header">
          <div>
            <p className="eyebrow">媒体导入</p>
            <h2>{isDragActive ? "松开即可导入媒体" : "导入图片与文档媒体"}</h2>
            <p className="muted-copy">
              支持 {supportedLabel}。导入会提交到本地 service，由 SQLite 账本记录状态与结果。
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
        ) : (
          <>
            <div className="media-import-drop-zone" data-active={isDragActive}>
              <div>
                <p className="eyebrow">导入入口</p>
                <h3>{isDragActive ? "释放文件开始预检" : "拖拽文件到这里"}</h3>
                <p className="muted-copy">
                  拖拽和按钮选择共用同一套类型预检、并发锁和逐文件反馈。
                </p>
              </div>
              <Button
                variant="primary"
                onClick={() => void handleChooseFiles()}
                disabled={isImporting}
              >
                {isImporting ? "导入中..." : "选择媒体"}
              </Button>
            </div>

            <div className="media-import-type-grid" aria-label="支持的媒体类型">
              {SUPPORTED_EXTENSIONS.map((ext) => (
                <span key={ext}>{ext.slice(1).toUpperCase()}</span>
              ))}
            </div>
          </>
        )}

        {error ? (
          <ErrorMessage
            title="媒体导入"
            message={error.message}
            correlationId={error.correlationId}
          />
        ) : null}
      </section>

      {workspaceReady && importResults.length === 0 ? (
        <EmptyState
          title="等待媒体文件"
          description="导入完成后，这里会显示成功、重复、不支持或失败的逐文件反馈。"
        />
      ) : null}

      {workspaceReady ? <ImportResultList results={importResults} /> : null}
    </div>
  );
}

function normalizeImportPaths(filePaths: unknown[]) {
  const uniquePaths = new Set<string>();
  for (const filePath of filePaths) {
    if (typeof filePath !== "string") {
      continue;
    }
    const trimmedPath = filePath.trim();
    if (trimmedPath.length > 0) {
      uniquePaths.add(trimmedPath);
    }
  }
  return [...uniquePaths];
}

function fileNameFromPath(path: string) {
  return path.split(/[/\\]/).pop() ?? path;
}

function getDroppedFilePath(file: File) {
  const fileWithPath = file as File & { path?: unknown };
  return typeof fileWithPath.path === "string" ? fileWithPath.path : null;
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
  return { message: "媒体导入失败，请稍后重试。" };
}
