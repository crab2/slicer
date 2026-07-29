interface ErrorMessageProps {
  title: string;
  message: string;
  details?: string | null;
  correlationId?: string | null;
}

export function ErrorMessage({ title, message, details, correlationId }: ErrorMessageProps) {
  return (
    <div className="error-message" role="status">
      <p className="error-title">{title}</p>
      <p className="error-copy">{message}</p>
      {details ? <p className="error-details">{details}</p> : null}
      {correlationId ? (
        <p className="error-correlation">诊断编号：{correlationId}</p>
      ) : null}
    </div>
  );
}
