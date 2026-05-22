import { useEffect, useMemo, useState } from "react";
import { Button } from "../../../components/common/Button";
import { EmptyState } from "../../../components/common/EmptyState";
import { ErrorMessage } from "../../../components/common/ErrorMessage";
import { StatusBadge } from "../../../components/common/StatusBadge";
import type { JobDto } from "../../../types/app";
import {
  filterJobsByStatus,
  JobPagination,
  JobStatusTabs,
  paginateJobs,
  type JobStatusFilter,
} from "./JobListControls";

const PAGE_SIZE = 4;

interface JobListProps {
  jobs: JobDto[];
  isLoading: boolean;
  isCreatingDemo: boolean;
  errorMessage: { message: string; correlationId?: string | null } | null;
  onCreateDemoJob: () => void;
  onRefresh: () => void;
}

export function JobList({
  jobs,
  isLoading,
  isCreatingDemo,
  errorMessage,
  onCreateDemoJob,
  onRefresh,
}: JobListProps) {
  const [activeStatus, setActiveStatus] = useState<JobStatusFilter>("all");
  const [page, setPage] = useState(1);
  const filteredJobs = useMemo(
    () => filterJobsByStatus(jobs, activeStatus),
    [jobs, activeStatus],
  );
  const pageCount = Math.max(1, Math.ceil(filteredJobs.length / PAGE_SIZE));
  const currentPage = Math.min(page, pageCount);
  const visibleJobs = useMemo(
    () => paginateJobs(filteredJobs, currentPage, PAGE_SIZE),
    [filteredJobs, currentPage],
  );

  useEffect(() => {
    setPage(1);
  }, [activeStatus]);

  useEffect(() => {
    setPage((current) => Math.min(current, pageCount));
  }, [pageCount]);

  return (
    <section className="panel panel-wide job-list-panel">
      <div className="panel-header">
        <div>
          <p className="eyebrow">任务编排</p>
          <h2>任务列表</h2>
          <p className="muted-copy">
            导入、分析和索引任务的执行记录与进度。
          </p>
        </div>
        <div className="action-row">
          <Button onClick={onRefresh} disabled={isLoading || isCreatingDemo}>
            {isLoading ? "刷新中" : "刷新"}
          </Button>
          <Button
            variant="primary"
            onClick={onCreateDemoJob}
            disabled={isLoading || isCreatingDemo}
          >
            {isCreatingDemo ? "创建中" : "创建示例任务"}
          </Button>
        </div>
      </div>

      {errorMessage ? <ErrorMessage title="任务列表暂不可用" message={errorMessage.message} correlationId={errorMessage.correlationId} /> : null}

      {jobs.length === 0 ? (
        <EmptyState
          title={isLoading ? "正在读取任务" : "暂无任务"}
          description="创建一个示例任务即可验证持久化任务结构与前端展示，不会触发真实业务处理。"
        />
      ) : (
        <>
          <JobStatusTabs
            jobs={jobs}
            activeStatus={activeStatus}
            onStatusChange={setActiveStatus}
            ariaLabel="筛选任务状态"
          />
          {visibleJobs.length === 0 ? (
            <p className="job-filter-empty">当前状态下暂无任务。</p>
          ) : (
            <div className="job-list" aria-label="任务列表">
              {visibleJobs.map((job) => (
                <article className="job-card" key={job.job_id}>
                  <div className="job-card-header">
                    <div>
                      <p className="eyebrow">任务类型</p>
                      <h3>{job.job_type}</h3>
                    </div>
                    <StatusBadge tone={statusTone(job.status)}>
                      {statusLabel(job.status)}
                    </StatusBadge>
                  </div>

                  <div className="job-meta">
                    <span>进度 {boundedProgress(job.progress)}%</span>
                    <span>更新于 {formatDateTime(job.updated_at)}</span>
                  </div>

                  <div
                    className="progress-track"
                    aria-label={`${job.job_type} 进度 ${boundedProgress(job.progress)}%`}
                  >
                    <span
                      className="progress-fill"
                      style={{ width: `${boundedProgress(job.progress)}%` }}
                    />
                  </div>

                  {job.error_summary ? (
                    <p className="job-error">失败摘要：{job.error_summary}</p>
                  ) : (
                    <p className="job-event">
                      {job.last_event_message ?? "任务已写入账本，等待后续服务更新。"}
                    </p>
                  )}
                </article>
              ))}
            </div>
          )}
          <JobPagination
            page={currentPage}
            pageCount={pageCount}
            pageSize={PAGE_SIZE}
            totalItems={filteredJobs.length}
            onPageChange={setPage}
          />
        </>
      )}
    </section>
  );
}

function boundedProgress(progress: number) {
  if (!Number.isFinite(progress)) {
    return 0;
  }
  return Math.min(100, Math.max(0, Math.round(progress)));
}

function formatDateTime(value: string) {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }
  return new Intl.DateTimeFormat("zh-CN", {
    dateStyle: "short",
    timeStyle: "medium",
  }).format(parsed);
}

function statusLabel(status: string) {
  switch (status) {
    case "queued":
      return "排队中";
    case "running":
      return "运行中";
    case "succeeded":
      return "已成功";
    case "failed":
      return "已失败";
    case "cancelled":
      return "已取消";
    default:
      return status;
  }
}

function statusTone(status: string) {
  switch (status) {
    case "succeeded":
      return "success";
    case "failed":
    case "cancelled":
      return "danger";
    case "running":
      return "warning";
    case "queued":
    default:
      return "neutral";
  }
}
