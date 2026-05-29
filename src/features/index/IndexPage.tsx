import { IndexStatusPanel } from "../search/components/IndexStatusPanel";

interface IndexPageProps {
  workspaceReady: boolean;
  isActive: boolean;
}

export function IndexPage({ workspaceReady, isActive }: IndexPageProps) {
  if (!workspaceReady) {
    return (
      <div className="panel">
        <p className="muted-copy">请先选择工作区。</p>
      </div>
    );
  }

  return (
    <div className="page-grid">
      <IndexStatusPanel workspaceReady={workspaceReady} isActive={isActive} />
    </div>
  );
}
