import { Button } from "../../../components/common/Button";
import { StatusBadge } from "../../../components/common/StatusBadge";
import type { WorkspaceStatusDto } from "../../../types/app";

interface WorkspaceSettingsProps {
  status: WorkspaceStatusDto;
  isLoading: boolean;
  onChooseWorkspace: () => void;
}

export function WorkspaceSettings({
  status,
  isLoading,
  onChooseWorkspace,
}: WorkspaceSettingsProps) {
  const ready = status.status === "ready";

  return (
    <section className="panel setting-row">
      <div>
        <h2>工作目录</h2>
        <p className="muted-copy">
          {ready ? status.workspace_path : "尚未选择工作区，选择后会在这里显示当前目录。"}
        </p>
        {status.error ? (
          <p className="settings-error">{status.error.message}</p>
        ) : status.status === "missing" ? (
          <p className="settings-error">上次使用的工作目录已不可访问，请重新选择。</p>
        ) : status.status === "invalid" ? (
          <p className="settings-error">所选目录不是有效的工作区，请重新选择。</p>
        ) : null}
      </div>
      <div className="setting-actions">
        <StatusBadge tone={ready ? "success" : "warning"}>{ready ? "可用" : "待选择"}</StatusBadge>
        <Button onClick={onChooseWorkspace} disabled={isLoading}>
          {ready ? "更改" : "选择"}
        </Button>
      </div>
    </section>
  );
}

