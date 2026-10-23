// React components of the OctopusOn SEO programs. They render the Deep Sea classes of
// @octopusseo/design and take every text as a prop, so each program keeps its own translations.

import { useEffect, useRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { AlertCircle, LoaderCircle, X } from "lucide-react";
import { Tip } from "./hint";

/** The three blocks of the OctopusOn SEO mark, in a 64-unit square (packages/design/brand). */
const PIECES = [
  "M31.95 3.93 L46.21 12.70 L46.21 28.72 L60.73 37.39 L49.34 43.59 L34.02 34.57 L34.07 18.85 L20.96 10.63 Z",
  "M17.69 12.39 L29.83 19.80 L29.83 34.67 L13.65 44.25 L13.71 59.01 L1.81 51.66 L1.86 38.05 L17.69 28.47 Z",
  "M31.95 38.10 L48.48 47.98 L62.19 40.27 L62.19 51.66 L48.08 60.07 L31.90 50.80 L16.28 59.62 L16.33 47.27 Z",
];

/** The OctopusOn SEO mark: three white glass blocks turning around a centre. */
export function BrandMark({ size }: { size?: number }) {
  return (
    <svg className="oc-brand-mark" viewBox="0 0 64 64" aria-hidden="true" width={size} height={size}>
      <defs>
        <linearGradient id="oc-mark-fill" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#ffffff" />
          <stop offset="0.55" stopColor="#eef1f6" />
          <stop offset="1" stopColor="#c3cad8" />
        </linearGradient>
        <linearGradient id="oc-mark-sheen" x1="0" y1="1" x2="1" y2="0">
          <stop offset="0.3" stopColor="#ffffff" stopOpacity="0" />
          <stop offset="0.5" stopColor="#ffffff" stopOpacity="0.55" />
          <stop offset="0.7" stopColor="#ffffff" stopOpacity="0" />
        </linearGradient>
      </defs>
      {PIECES.map((d) => (
        <g key={d}>
          <path d={d} fill="url(#oc-mark-fill)" />
          <path d={d} fill="url(#oc-mark-sheen)" />
        </g>
      ))}
    </svg>
  );
}

/** A button: white `primary`, glass otherwise; `busy` shows a spinner and disables it. */
export function Button({
  primary = false,
  small = false,
  busy = false,
  className = "",
  children,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { primary?: boolean; small?: boolean; busy?: boolean }) {
  return (
    <button
      type="button"
      className={`oc-btn ${primary ? "primary" : ""} ${small ? "small" : ""} ${className}`.trim()}
      {...props}
      disabled={props.disabled || busy}
    >
      {busy && <LoaderCircle size={15} className="oc-spin" aria-hidden="true" />}
      {children}
    </button>
  );
}

/**
 * A dialog that keeps the focus inside, closes on Escape and gives the focus back. It is drawn on
 * the document body: glass panels (backdrop filters) would otherwise trap its fixed position.
 */
export function Modal({
  title,
  closeLabel,
  onClose,
  children,
  footer,
  className = "",
}: {
  title: string;
  closeLabel: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  className?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const controls = () =>
      Array.from(
        ref.current?.querySelectorAll<HTMLElement>('button:not(:disabled),input,select,textarea,[tabindex="0"]') ?? [],
      ).filter((element) => !element.hidden && element.getClientRects().length > 0);
    (controls().find((element) => ["INPUT", "TEXTAREA", "SELECT"].includes(element.tagName)) ?? controls()[0])?.focus();
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
      if (event.key !== "Tab") return;
      const list = controls();
      const first = list[0];
      const last = list.at(-1);
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last?.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first?.focus();
      }
    };
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("keydown", key);
      previous?.focus();
    };
  }, [onClose]);
  return createPortal(
    <div className="oc-scrim" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div ref={ref} role="dialog" aria-modal="true" aria-label={title} className={`oc-modal ${className}`.trim()}>
        <header className="ui-modal-head">
          <h2>{title}</h2>
          <button type="button" className="oc-btn icon ghost" onClick={onClose} aria-label={closeLabel}>
            <X size={19} />
          </button>
        </header>
        <div className="ui-modal-body">{children}</div>
        {footer && <footer className="ui-modal-foot">{footer}</footer>}
      </div>
    </div>,
    document.body,
  );
}

/** A labelled field. */
export function Field({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <label className="oc-field">
      <span>{label}</span>
      {children}
      {hint && <small className="oc-field-hint">{hint}</small>}
    </label>
  );
}

/** Nothing to show yet, with what to do about it. */
export function Empty({ icon, title, children }: { icon?: ReactNode; title?: string; children?: ReactNode }) {
  return (
    <div className="oc-empty ui-empty">
      {icon}
      {title && <strong>{title}</strong>}
      {children && <div>{children}</div>}
    </div>
  );
}

/** Loading, announced to screen readers. */
export function Loading({ label }: { label: string }) {
  return (
    <div className="ui-empty" role="status">
      <LoaderCircle className="oc-spin" size={24} aria-hidden="true" />
      {label}
    </div>
  );
}

/** A failure with an optional way to try again. */
export function ErrorState({
  title,
  message,
  retry,
  retryLabel,
}: {
  title: string;
  message: string;
  retry?: () => void;
  retryLabel?: string;
}) {
  return (
    <div className="oc-banner error" role="alert">
      <AlertCircle size={18} aria-hidden="true" />
      <div>
        <b>{title}</b>
        <p>{message}</p>
      </div>
      {retry && retryLabel && (
        <Button small onClick={retry}>
          {retryLabel}
        </Button>
      )}
    </div>
  );
}

/** One number with its label, an optional hint and an optional explanation beside the label. */
export function Stat({ label, value, hint, tone, help, helpLabel }: { label: string; value: ReactNode; hint?: ReactNode; tone?: "good" | "bad"; help?: string; helpLabel?: string }) {
  return (
    <div className={`oc-stat ${tone ?? ""}`.trim()}>
      <span className="oc-stat-label">
        {label}
        {help && <Tip text={help} label={helpLabel ?? label} />}
      </span>
      <strong className="oc-stat-value">{value}</strong>
      {hint && <span className="oc-stat-hint">{hint}</span>}
    </div>
  );
}

/** A segmented choice between a few values. */
export function Segmented<T extends string>({
  label,
  value,
  options,
  onChange,
  disabled = false,
}: {
  label: string;
  value: T;
  options: { value: T; label: string; disabled?: boolean }[];
  onChange: (value: T) => void;
  disabled?: boolean;
}) {
  return (
    <div className="oc-segmented" role="radiogroup" aria-label={label}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          role="radio"
          aria-checked={value === option.value}
          className={value === option.value ? "active" : undefined}
          disabled={disabled || option.disabled}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

export { DataTable, columnShown, sortRows, type Column, type SortDirection, type TableLabels } from "./table";
export { Tip } from "./hint";
