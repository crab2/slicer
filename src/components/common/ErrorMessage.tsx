interface ErrorMessageProps {
  title: string;
  message: string;
  correlationId?: string | null;
}

export function ErrorMessage({ title, message, correlationId }: ErrorMessageProps) {
  return (
    <div className="error-message" role="status">
      <p className="error-title">{title}</p>
      <p className="error-copy">{message}</p>
      {correlationId ? (
        <p className="error-correlation">诊断编号：{correlationId}</p>
      ) : null}
    </div>
  );
}
