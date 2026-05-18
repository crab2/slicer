import { useEffect, useMemo, useState } from "react";
import { EmptyState } from "../components/common/EmptyState";
import { ErrorMessage } from "../components/common/ErrorMessage";
import { StatusBadge } from "../components/common/StatusBadge";
import { SearchPage } from "../features/search/SearchPage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { WorkbenchPage } from "../features/workbench/WorkbenchPage";
import { tauriClient } from "../lib/tauriClient";
import type { WorkspaceStatusDto } from "../types/app";
import { navigationItems, type ViewId } from "./navigation";

const pageTitles: Record<ViewId, string> = {
  workbench: "工作台",
  search: "搜索",
  settings: "设置",
};

const initialWorkspaceStatus: WorkspaceStatusDto = {
  status: "loading",
  workspace_path: null,
  error: null,
};

export function AppShell() {
  const [activeView, setActiveView] = useState<ViewId>("workbench");
  const [workspaceStatus, setWorkspaceStatus] =
    useState<WorkspaceStatusDto>(initialWorkspaceStatus);
  const [isWorkspaceLoading, setIsWorkspaceLoading] = useState(true);

  async function refreshWorkspaceStatus() {
    setIsWorkspaceLoading(true);
    try {
      setWorkspaceStatus(await tauriClient.getWorkspaceStatus());
    } catch (error) {
      setWorkspaceStatus(commandFailureStatus(error));
    } finally {
      setIsWorkspaceLoading(false);
    }
  }

  async function handleChooseWorkspace() {
    setIsWorkspaceLoading(true);
    try {
      const result = await tauriClient.openWorkspaceDialog();
      setWorkspaceStatus(result);
    } catch (error) {
      setWorkspaceStatus(commandFailureStatus(error));
    } finally {
      setIsWorkspaceLoading(false);
    }
  }

  useEffect(() => {
    void refreshWorkspaceStatus();
  }, []);

  const currentView = useMemo(() => {
    switch (activeView) {
      case "search":
        return <SearchPage />;
      case "settings":
        return (
          <SettingsPage
            workspaceStatus={workspaceStatus}
            isWorkspaceLoading={isWorkspaceLoading}
            onChooseWorkspace={handleChooseWorkspace}
          />
        );
      case "workbench":
      default:
        return (
          <WorkbenchPage
            workspaceStatus={workspaceStatus}
            isWorkspaceLoading={isWorkspaceLoading}
            onChooseWorkspace={handleChooseWorkspace}
          />
        );
    }
  }, [activeView, isWorkspaceLoading, workspaceStatus]);

  const workspaceReady = workspaceStatus.status === "ready";
  const workspaceIssue = workspaceStatus.error?.message;
  const sidebarWorkspacePath = workspaceStatus.workspace_path;

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="主导航">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            <span className="logo-ring" />
            <span className="logo-slice" />
          </span>
          <div>
            <p className="brand-name">SLICER</p>
            <p className="brand-subtitle">本地文档处理工作台</p>
          </div>
        </div>

        <nav className="nav-list">
          {navigationItems.map((item) => (
            <button
              className="nav-item"
              data-active={activeView === item.id}
              key={item.id}
              onClick={() => setActiveView(item.id)}
              type="button"
            >
              {item.label}
            </button>
          ))}
        </nav>

        <div className="sidebar-footer">
          <StatusBadge tone={workspaceReady ? "success" : "neutral"}>
            {workspaceReady ? "工作区可用" : "尚未选择工作区"}
          </StatusBadge>
          {sidebarWorkspacePath ? <p className="sidebar-path">{sidebarWorkspacePath}</p> : null}
        </div>
      </aside>

      <main className="main-area">
        <header className="topbar">
          <div>
            <p className="eyebrow">当前视图</p>
            <h1>{pageTitles[activeView]}</h1>
          </div>
          <StatusBadge tone={workspaceReady ? "success" : "warning"}>
            {workspaceReady ? "工作区已恢复" : "工作区待配置"}
          </StatusBadge>
        </header>

        {workspaceIssue ? (
          <ErrorMessage message={workspaceIssue} title="工作区状态" />
        ) : null}

        <section className="content-area">{currentView}</section>

        <EmptyState
          title="加载中状态预留"
          description="后续任务编排、索引重建和模型分析会在这里接入统一的加载与失败状态。"
        />
      </main>
    </div>
  );
}

function commandFailureStatus(error: unknown): WorkspaceStatusDto {
  const message = error instanceof Error ? error.message : "后端命令调用失败，请查看诊断信息。";
  return {
    status: "error",
    workspace_path: null,
    error: {
      code: "tauri_command_failed",
      message,
      stage: "workspace",
      retryable: true,
      details: null,
      correlation_id: "frontend",
    },
  };
}
