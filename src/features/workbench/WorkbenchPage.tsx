import { EmptyState } from "../../components/common/EmptyState";
import { StatusBadge } from "../../components/common/StatusBadge";
import type { WorkspaceStatusDto } from "../../types/app";
import { WorkspacePicker } from "./components/WorkspacePicker";

const placeholderTasks = ["选择工作区", "导入文件", "任务列表"];

interface WorkbenchPageProps {
  workspaceStatus: WorkspaceStatusDto;
  isWorkspaceLoading: boolean;
  onChooseWorkspace: () => void;
}

export function WorkbenchPage({
  workspaceStatus,
  isWorkspaceLoading,
  onChooseWorkspace,
}: WorkbenchPageProps) {
  const workspaceReady = workspaceStatus.status === "ready";

  return (
    <div className="page-grid">
      <section className="panel panel-wide">
        <div className="panel-header">
          <div>
            <p className="eyebrow">应用名称</p>
            <h2>SLICER</h2>
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
        {workspaceReady ? null : (
          <EmptyState
            title="等待工作区"
            description="导入、转换、分析和索引任务会在工作区可用后接入。"
          />
        )}
      </section>

      {placeholderTasks.map((title) => (
        <section className="panel" key={title}>
          <div className="panel-header compact">
            <h3>{title}</h3>
            <StatusBadge>待接入</StatusBadge>
          </div>
          <p className="muted-copy">功能待接入。当前仅保留稳定入口，不执行真实业务操作。</p>
        </section>
      ))}
    </div>
  );
}
