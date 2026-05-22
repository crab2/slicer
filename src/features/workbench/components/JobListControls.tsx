import { Button } from "../../../components/common/Button";
import type { JobDto } from "../../../types/app";

export type JobStatusFilter =
  | "all"
  | "queued"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled";

const STATUS_FILTERS: Array<{ value: JobStatusFilter; label: string }> = [
  { value: "all", label: "全部" },
  { value: "running", label: "运行中" },
  { value: "queued", label: "排队中" },
  { value: "succeeded", label: "已成功" },
  { value: "failed", label: "已失败" },
  { value: "cancelled", label: "已取消" },
];

interface JobStatusTabsProps {
  jobs: JobDto[];
  activeStatus: JobStatusFilter;
  onStatusChange: (status: JobStatusFilter) => void;
  ariaLabel: string;
}

interface JobPaginationProps {
  page: number;
  pageCount: number;
  pageSize: number;
  totalItems: number;
  onPageChange: (page: number) => void;
}

export function JobStatusTabs({
  jobs,
  activeStatus,
  onStatusChange,
  ariaLabel,
}: JobStatusTabsProps) {
  const counts = countJobsByStatus(jobs);

  return (
    <div className="job-status-tabs" role="tablist" aria-label={ariaLabel}>
      {STATUS_FILTERS.map((filter) => {
        const count = filter.value === "all" ? jobs.length : counts[filter.value];
        const selected = activeStatus === filter.value;
        return (
          <button
            type="button"
            className="job-status-tab"
            data-active={selected}
            role="tab"
            aria-selected={selected}
            key={filter.value}
            onClick={() => onStatusChange(filter.value)}
          >
            <span>{filter.label}</span>
            <span className="job-status-count">{count}</span>
          </button>
        );
      })}
    </div>
  );
}

export function JobPagination({
  page,
  pageCount,
  pageSize,
  totalItems,
  onPageChange,
}: JobPaginationProps) {
  if (totalItems <= pageSize) {
    return null;
  }

  const firstItem = (page - 1) * pageSize + 1;
  const lastItem = Math.min(totalItems, page * pageSize);

  return (
    <div className="job-pagination" aria-label="分页">
      <p className="job-pagination-copy">
        {firstItem}-{lastItem} / {totalItems}
      </p>
      <div className="job-pagination-actions">
        <Button onClick={() => onPageChange(page - 1)} disabled={page <= 1}>
          上一页
        </Button>
        <span className="job-page-indicator">
          {page} / {pageCount}
        </span>
        <Button
          onClick={() => onPageChange(page + 1)}
          disabled={page >= pageCount}
        >
          下一页
        </Button>
      </div>
    </div>
  );
}

export function filterJobsByStatus(
  jobs: JobDto[],
  status: JobStatusFilter,
) {
  if (status === "all") {
    return jobs;
  }
  return jobs.filter((job) => job.status === status);
}

export function paginateJobs(jobs: JobDto[], page: number, pageSize: number) {
  return jobs.slice((page - 1) * pageSize, page * pageSize);
}

function countJobsByStatus(jobs: JobDto[]) {
  return {
    queued: jobs.filter((job) => job.status === "queued").length,
    running: jobs.filter((job) => job.status === "running").length,
    succeeded: jobs.filter((job) => job.status === "succeeded").length,
    failed: jobs.filter((job) => job.status === "failed").length,
    cancelled: jobs.filter((job) => job.status === "cancelled").length,
  };
}
