import {
  useEffect,
  useRef,
  type ButtonHTMLAttributes,
  type ReactNode,
} from "react";
import { AlertCircle, LoaderCircle, X } from "lucide-react";
import { useI18n } from "../i18n";

export function Button({
  primary = false,
  small = false,
  busy = false,
  className = "",
  children,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  primary?: boolean;
  small?: boolean;
  busy?: boolean;
}) {
  return (
    <button
      className={`oc-btn ${primary ? "primary" : ""} ${small ? "small" : ""} ${className}`}
      {...props}
      disabled={props.disabled || busy}
    >
      {busy && <LoaderCircle size={15} className="spin" />}
      {children}
    </button>
  );
}
export function Modal({
  title,
  className = "",
  onClose,
  children,
  footer,
}: {
  title: string;
  className?: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  const { t } = useI18n();
  useEffect(() => { closeRef.current = onClose; }, [onClose]);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const controls = () =>
      Array.from(
        ref.current?.querySelectorAll<HTMLElement>(
          'button:not(:disabled),input,select,textarea,[tabindex="0"]',
        ) ?? [],
      ).filter(element=>!element.hidden&&element.getClientRects().length>0);
    (
      controls().find(element=>["INPUT","TEXTAREA","SELECT"].includes(element.tagName)&&!element.matches(":disabled")) ??
      controls()[0]
    )?.focus();
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") closeRef.current();
      if (e.key === "Tab") {
        const list = controls();
        const first = list[0],
          last = list.at(-1);
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last?.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first?.focus();
        }
      }
    };
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("keydown", key);
      previous?.focus();
    };
  }, []);
  return (
    <div
      className="oc-scrim"
      onMouseDown={(e) => e.target === e.currentTarget && onClose()}
    >
      <div
        ref={ref}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className={`oc-modal spider-modal ${className}`}
      >
        <header className="row between">
          <h2>{title}</h2>
          <Button
            className="icon ghost"
            onClick={onClose}
            aria-label={t("close")}
          >
            <X size={19} />
          </Button>
        </header>
        <div className="modal-content">{children}</div>
        {footer && <footer className="modal-footer row end">{footer}</footer>}
      </div>
    </div>
  );
}
export function Empty({
  icon,
  children,
}: {
  icon?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="sp-empty">
      {icon}
      <div>{children}</div>
    </div>
  );
}
export function Loading() {
  const { t } = useI18n();
  return (
    <div className="sp-empty" role="status">
      <LoaderCircle className="spin" size={24} />
      {t("loading")}
    </div>
  );
}
export function ErrorState({
  message,
  retry,
}: {
  message: string;
  retry?: () => void;
}) {
  const { t } = useI18n();
  return (
    <div className="oc-banner error">
      <AlertCircle size={18} />
      <div>
        <b>{t("loadFailed")}</b>
        <p>{message}</p>
      </div>
      {retry && (
        <Button small onClick={retry}>
          {t("retry")}
        </Button>
      )}
    </div>
  );
}
