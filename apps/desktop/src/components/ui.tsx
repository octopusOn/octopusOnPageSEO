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
    "btn",
    variant === "danger-solid" ? "danger solid" : variant === "default" ? "" : variant,
    small ? "small" : "",
    children ? "" : "icon",
    className ?? "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <button type="button" className={classes} disabled={busy || props.disabled} {...props}>
      {busy ? <span className="spinner" /> : icon}
      {children}
    </button>
  );
}

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="field">
      <span>{label}</span>
      {children}
    </label>
  );
}

export function TextInput(props: InputHTMLAttributes<HTMLInputElement>) {
  return <input className="input" {...props} />;
}

export function Select({ children, ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <select className="select" {...props}>
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
    <label className="check">
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
    <div className="segmented">
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
    <section className="card">
      {(title || actions) && (
        <div className="card-title">
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
    <div className={`kpi ${tone ?? ""}`}>
      <div className="kpi-label">{label}</div>
      <div className="kpi-value">{value}</div>
      {hint && <div className="kpi-hint">{hint}</div>}
    </div>
  );
}

export function Badge({ tone, children }: { tone?: "success" | "warning" | "danger" | "accent"; children: ReactNode }) {
  return <span className={`badge ${tone ?? ""}`}>{children}</span>;
}

export function Spinner() {
  return <span className="spinner" />;
}

export function Empty({ icon, title, children }: { icon?: ReactNode; title: string; children?: ReactNode }) {
  return (
    <div className="empty">
      {icon}
      <h2>{title}</h2>
      {children}
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
    <div className="banner error" role="alert">
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
