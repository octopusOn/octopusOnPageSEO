// Small presentational building blocks.

import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode, SelectHTMLAttributes } from "react";
import { AlertTriangle } from "lucide-react";

import type { CommandError } from "../api/types";
import { useI18n } from "../i18n";

type ButtonVariant = "default" | "primary" | "ghost" | "danger" | "danger-solid";

export function Button({
  variant = "default",
  small,
  icon,
  busy,
  children,
  className,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  small?: boolean;
  icon?: ReactNode;
  busy?: boolean;
}) {
  const classes = [
    "oc-btn",
    variant === "danger-solid" ? "danger solid" : variant === "default" ? "" : variant,
    small ? "small" : "",
    children ? "" : "icon",
    className ?? "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <button type="button" className={classes} disabled={busy || props.disabled} {...props}>
      {busy ? <span className="oc-spinner" /> : icon}
      {children}
    </button>
  );
}

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="oc-field">
      <span>{label}</span>
      {children}
    </label>
  );
}

export function TextInput(props: InputHTMLAttributes<HTMLInputElement>) {
  return <input className="oc-input" {...props} />;
}

export function Select({ children, ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <select className="oc-select" {...props}>
      {children}
    </select>
  );
}

export function Checkbox({
  checked,
  onChange,
  children,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  children: ReactNode;
}) {
  return (
    <label className="oc-check">
      <input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
      {children}
    </label>
  );
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
}) {
  return (
    <div className="oc-segmented">
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          className={option.value === value ? "active" : ""}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

export function Card({ title, actions, children }: { title?: ReactNode; actions?: ReactNode; children: ReactNode }) {
  return (
    <section className="oc-panel">
      {(title || actions) && (
        <div className="oc-panel-head">
          {typeof title === "string" ? <h2>{title}</h2> : <div className="grow">{title}</div>}
          {actions}
        </div>
      )}
      {children}
    </section>
  );
}

export function Kpi({
  label,
  value,
  hint,
  tone,
}: {
  label: string;
  value: string;
  hint?: string;
  tone?: "danger" | "success";
}) {
  return (
    <div className={`oc-stat ${tone ? STAT_TONES[tone] : ""}`}>
      <div className="oc-stat-label">{label}</div>
      <div className="oc-stat-value">{value}</div>
      {hint && <div className="oc-stat-hint">{hint}</div>}
    </div>
  );
}

const STAT_TONES = { danger: "critical", success: "good" } as const;

const BADGE_TONES = { success: "good", warning: "warning", danger: "critical", accent: "accent", product: "product" } as const;

export function Badge({ tone, children }: { tone?: keyof typeof BADGE_TONES; children: ReactNode }) {
  return <span className={`oc-badge ${tone ? BADGE_TONES[tone] : ""}`}>{children}</span>;
}

export function Spinner() {
  return <span className="oc-spinner" />;
}

/** A centered empty state; the icon sits in the radar emblem, which sweeps while `scanning`. */
export function Empty({
  icon,
  title,
  scanning,
  children,
}: {
  icon?: ReactNode;
  title: string;
  scanning?: boolean;
  children?: ReactNode;
}) {
  return (
    <div className="oc-empty">
      {icon && <div className={`oc-radar ${scanning ? "scanning" : ""}`}>{icon}</div>}
      <h2>{title}</h2>
      {children}
    </div>
  );
}

/** Full-page loading state: the radar sweep. */
export function PageLoader({ label }: { label?: string }) {
  return (
    <div className="oc-empty" role="status" aria-live="polite">
      <div className="oc-radar scanning" />
      {label && <p className="muted">{label}</p>}
    </div>
  );
}

/** Shows a command error; syntax errors underline the offending part of the query. */
export function ErrorBanner({ error }: { error: CommandError | null | undefined }) {
  const { t } = useI18n();
  if (!error) return null;
  const title =
    error.kind === "syntax" ? t("error.syntax") : error.kind === "busy" ? t("error.busy") : t("error.generic");
  return (
    <div className="oc-banner error" role="alert">
      <AlertTriangle size={18} />
      <div className="grow">
        <strong>{title}:</strong> {error.kind === "busy" ? t("site.busy") : error.message}
        {error.kind === "syntax" && error.query !== undefined && error.span && (
          <pre>{highlight(error.query, error.span)}</pre>
        )}
      </div>
    </div>
  );
}

function highlight(text: string, [start, end]: [number, number]): ReactNode {
  // Spans are byte offsets; the query is mostly ASCII, so map via UTF-8 encoding.
  const bytes = new TextEncoder().encode(text);
  const decoder = new TextDecoder();
  const before = decoder.decode(bytes.slice(0, start));
  const marked = decoder.decode(bytes.slice(start, Math.max(end, start)));
  const after = decoder.decode(bytes.slice(Math.max(end, start)));
  return (
    <>
      {before}
      <mark>{marked || " "}</mark>
      {after}
    </>
  );
}
