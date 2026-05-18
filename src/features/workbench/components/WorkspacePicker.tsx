import { Button } from "../../../components/common/Button";
import { ErrorMessage } from "../../../components/common/ErrorMessage";
import { StatusBadge } from "../../../components/common/StatusBadge";
import type { WorkspaceStatusDto } from "../../../types/app";

interface WorkspacePickerProps {
  status: WorkspaceStatusDto;
  isLoading: boolean;
  onChooseWorkspace: () => void;
}

export function WorkspacePicker({
  status,
  isLoading,
  onChooseWorkspace,
}: WorkspacePickerProps) {
  const ready = status.status === "ready";
  const title = ready ? "当前工作区" : "尚未选择工作区";
  const description = ready
    ? status.workspace_path ?? "工作区路径不可用"
    : "选择本地目录后，SLICER 会初始化标准工作区结构并在下次启动时恢复。";

  return (
    <div className="workspace-picker">
      <div className="workspace-summary">
        <div>
          <p className="eyebrow">工作目录</p>
          <h3>{title}</h3>
          <p className="path-copy">{description}</p>
        </div>
        <StatusBadge tone={ready ? "success" : "warning"}>{workspaceStatusLabel(status)}</StatusBadge>
      </div>

      {status.error ? (
        <ErrorMessage title="工作区需要处理" message={status.error.message} />
      ) : null}

      <div className="action-row workbench-actions">
        <Button variant="primary" onClick={onChooseWorkspace} disabled={isLoading}>
          {ready ? "更改工作区" : "选择工作区"}
        </Button>
      </div>
    </div>
  );
}

function workspaceStatusLabel(status: WorkspaceStatusDto) {
  switch (status.status) {
    case "ready":
      return "可用";
    case "missing":
      return "目录缺失";
    case "invalid":
      return "目录无效";
    case "loading":
      return "加载中";
    case "error":
      return "需要处理";
    case "not_selected":
    default:
      return "未选择";
  }
}

