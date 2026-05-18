import { useEffect, useRef, useState } from "react";
import { EmptyState } from "../../components/common/EmptyState";
import { StatusBadge } from "../../components/common/StatusBadge";
import { tauriClient } from "../../lib/tauriClient";
import type { JobDto, WorkspaceStatusDto } from "../../types/app";
import { JobList } from "./components/JobList";
import { WorkspacePicker } from "./components/WorkspacePicker";

const placeholderTasks = ["导入文件", "页面分析", "索引构建"];
const demoJobType = "placeholder_import";

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
  const workspaceKey = workspaceStatus.workspace_path ?? "current";
  const recoveredWorkspaceRef = useRef<string | null>(null);
  const [jobs, setJobs] = useState<JobDto[]>([]);
  const [isJobsLoading, setIsJobsLoading] = useState(false);
  const [isCreatingDemo, setIsCreatingDemo] = useState(false);
  const [jobsError, setJobsError] = useState<string | null>(null);

  async function refreshJobs(options: { recoverInterrupted?: boolean } = {}) {
    if (!workspaceReady) {
      setJobs([]);
      return;
    }

    setIsJobsLoading(true);
    setJobsError(null);
    try {
      if (options.recoverInterrupted) {
        await tauriClient.recoverInterruptedJobs();
        recoveredWorkspaceRef.current = workspaceKey;
      }
      setJobs(await tauriClient.listJobs());
    } catch (error) {
      setJobsError(commandErrorMessage(error));
    } finally {
      setIsJobsLoading(false);
    }
  }

  async function handleCreateDemoJob() {
    setIsCreatingDemo(true);
    setJobsError(null);
    try {
      await tauriClient.createJob(demoJobType);
      await refreshJobs();
    } catch (error) {
      setJobsError(commandErrorMessage(error));
    } finally {
      setIsCreatingDemo(false);
    }
  }

  useEffect(() => {
    if (!workspaceReady) {
      setJobs([]);
      setJobsError(null);
      setIsJobsLoading(false);
      return;
    }

    void refreshJobs({
      recoverInterrupted: recoveredWorkspaceRef.current !== workspaceKey,
    });
  }, [workspaceReady, workspaceKey]);

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

      {workspaceReady ? (
        <JobList
          jobs={jobs}
          isLoading={isJobsLoading}
          isCreatingDemo={isCreatingDemo}
          errorMessage={jobsError}
          onCreateDemoJob={handleCreateDemoJob}
          onRefresh={() => void refreshJobs()}
        />
      ) : null}

      {placeholderTasks.map((title) => (
        <section className="panel" key={title}>
          <div className="panel-header compact">
            <h3>{title}</h3>
            <StatusBadge>待接入</StatusBadge>
          </div>
          <p className="muted-copy">
            功能待接入。当前仅保留稳定入口，不执行真实业务操作。
          </p>
        </section>
      ))}
    </div>
  );
}

function commandErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "object" && error !== null && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string") {
      return message;
    }
  }
  if (typeof error === "string") {
    return error;
  }
  return "任务命令调用失败，请稍后重试。";
}
