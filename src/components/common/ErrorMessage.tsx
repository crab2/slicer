interface ErrorMessageProps {
  title: string;
  message: string;
}

export function ErrorMessage({ title, message }: ErrorMessageProps) {
  return (
    <div className="error-message" role="status">
      <p className="error-title">{title}</p>
      <p className="error-copy">{message}</p>
    </div>
  );
}
