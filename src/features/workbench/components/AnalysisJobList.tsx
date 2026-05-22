import { useEffect, useMemo, useState } from "react";
import { StatusBadge } from "../../../components/common/StatusBadge";
import type { JobDto } from "../../../types/app";
import {
  filterJobsByStatus,
  JobPagination,
  JobStatusTabs,
  paginateJobs,
  type JobStatusFilter,
} from "./JobListControls";

const ANALYSIS_JOB_TYPES = new Set([
  "page_analysis",
  "page_analysis_batch",
  "document_reanalysis",
  "document_failed_reanalysis",
]);

const PAGE_SIZE = 3;

interface AnalysisJobListProps {
  jobs: JobDto[];
}

export function isAnalysisJobType(jobType: string) {
  return ANALYSIS_JOB_TYPES.has(jobType);
}

export function AnalysisJobList({ jobs }: AnalysisJobListProps) {
  const analysisJobs = jobs.filter((job) => ANALYSIS_JOB_TYPES.has(job.job_type));
  const [activeStatus, setActiveStatus] = useState<JobStatusFilter>("all");
  const [page, setPage] = useState(1);
  const filteredJobs = useMemo(
    () => filterJobsByStatus(analysisJobs, activeStatus),
    [analysisJobs, activeStatus],
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

  if (analysisJobs.length === 0) {
    return null;
  }

  return (
    <section className="panel panel-wide analysis-job-panel">
      <div className="panel-header">
        <div>
          <p className="eyebrow">分析任务</p>
          <h2>任务进度</h2>
          <p className="muted-copy">
            状态来自 SQLite 账本，切换视图后刷新可恢复最新进度。
          </p>
        </div>
      </div>
      <JobStatusTabs
        jobs={analysisJobs}
        activeStatus={activeStatus}
        onStatusChange={setActiveStatus}
        ariaLabel="筛选分析任务状态"
      />
      {visibleJobs.length === 0 ? (
        <p className="job-filter-empty">当前状态下暂无分析任务。</p>
      ) : (
        <AnalysisJobCards jobs={visibleJobs} />
      )}
      <JobPagination
        page={currentPage}
        pageCount={pageCount}
        pageSize={PAGE_SIZE}
        totalItems={filteredJobs.length}
        onPageChange={setPage}
      />
    </section>
  );
}

function AnalysisJobCards({ jobs }: { jobs: JobDto[] }) {
  return (
    <div className="job-list" aria-label="分析任务列表">
      {jobs.map((job) => {
        const progress = boundedProgress(job.progress);
        return (
          <article className="job-card" key={job.job_id}>
            <div className="job-card-header">
              <div>
                <p className="eyebrow">任务类型</p>
                <h3>{jobTypeLabel(job.job_type)}</h3>
              </div>
              <StatusBadge tone={statusTone(job.status)}>
                {statusLabel(job.status)}
              </StatusBadge>
            </div>
            <div className="job-meta">
              <span>进度 {progress}%</span>
              <span>更新于 {formatDateTime(job.updated_at)}</span>
            </div>
            <div
              className="progress-track"
              aria-label={`${jobTypeLabel(job.job_type)} 进度 ${progress}%`}
            >
              <span className="progress-fill" style={{ width: `${progress}%` }} />
            </div>
            {job.error_summary ? (
              <p className="job-error">失败摘要：{job.error_summary}</p>
            ) : (
              <p className="job-event">
                {job.last_event_message ?? "分析任务已写入账本。"}
              </p>
            )}
          </article>
        );
      })}
    </div>
  );
}

function jobTypeLabel(jobType: string) {
  switch (jobType) {
    case "page_analysis":
      return "单页分析";
    case "page_analysis_batch":
      return "新页面批量分析";
    case "document_reanalysis":
      return "文档重新分析";
    case "document_failed_reanalysis":
      return "失败页面重分析";
    default:
      return jobType;
  }
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
    default:
      return "neutral";
  }
}
