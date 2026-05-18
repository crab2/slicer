import type { PropsWithChildren } from "react";

type StatusTone = "neutral" | "warning" | "success";

interface StatusBadgeProps {
  tone?: StatusTone;
}

export function StatusBadge({ children, tone = "neutral" }: PropsWithChildren<StatusBadgeProps>) {
  return (
    <span className="status-badge" data-tone={tone}>
      {children}
    </span>
  );
}
