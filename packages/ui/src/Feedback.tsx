import type { ReactNode } from "react";

export interface FeedbackPanelProps {
  tone: "neutral" | "success" | "warning" | "danger";
  title: string;
  children?: ReactNode;
  action?: ReactNode;
  busy?: boolean;
}

export function FeedbackPanel({
  tone,
  title,
  children,
  action,
  busy = false,
}: FeedbackPanelProps) {
  return (
    <section
      className={`crm-feedback crm-feedback-${tone}`}
      aria-live={tone === "danger" ? "assertive" : "polite"}
      aria-busy={busy || undefined}
    >
      <div>
        <h2>{title}</h2>
        {children ? <div className="crm-feedback-body">{children}</div> : null}
      </div>
      {action ? <div className="crm-feedback-action">{action}</div> : null}
    </section>
  );
}

export interface ButtonProps {
  children: ReactNode;
  onClick?: () => void;
  type?: "button" | "submit" | "reset";
  disabled?: boolean;
  variant?: "primary" | "secondary" | "danger";
}

export function Button({
  children,
  onClick,
  type = "button",
  disabled = false,
  variant = "primary",
}: ButtonProps) {
  return (
    <button
      className={`crm-button crm-button-${variant}`}
      type={type}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
