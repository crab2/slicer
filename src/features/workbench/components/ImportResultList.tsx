import type { ImportResultDto } from "../../../types/app";

interface ImportResultListProps {
  results: ImportResultDto[];
}

export function ImportResultList({ results }: ImportResultListProps) {
  if (results.length === 0) return null;

  return (
    <div className="import-result-list">
      {results.map((r, i) => (
        <div key={`${r.file_name}-${i}`} className="import-result-item">
          <span className="import-result-name">{r.file_name}</span>
          <span className={`import-result-badge import-result-${r.status}`}>
            {statusLabel(r.status)}
          </span>
          {r.error ? <span className="import-result-error">{r.error}</span> : null}
        </div>
      ))}
    </div>
  );
}

function statusLabel(status: string) {
  switch (status) {
    case "success":
      return "已导入";
    case "duplicate":
      return "已存在";
    case "unsupported":
      return "不支持";
    case "failed":
      return "失败";
    default:
      return status;
  }
}
