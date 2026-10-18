import { Info } from "lucide-react";
import type { Breakdown, ScoreComponent } from "../api/types";
import { hasKey, number, percent, useI18n, type Key } from "../i18n";
import { ordered, reasonKeys } from "../lib/score";

/** A 0–100 score as a ring, coloured by `tone`. */
export function ScoreRing({
  value,
  label,
  size = 132,
  tone,
}: {
  value: number;
  label: string;
  size?: number;
  tone: string;
}) {
  const { lang } = useI18n();
  const clamped = Math.max(0, Math.min(100, value));
  const radius = 42;
  const circumference = 2 * Math.PI * radius;
  // Three quarters of a circle, open at the bottom.
  const arc = circumference * 0.75;
  return (
    <div
      className={`score-ring ${tone}`}
      style={{ width: size, height: size }}
      role="meter"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Number(clamped.toFixed(1))}
    >
      <svg viewBox="0 0 100 100" aria-hidden="true">
        <circle
          className="score-ring-track"
          cx="50"
          cy="50"
          r={radius}
          strokeDasharray={`${arc} ${circumference}`}
          transform="rotate(135 50 50)"
        />
        <circle
          className="score-ring-value"
          cx="50"
          cy="50"
          r={radius}
          strokeDasharray={`${(arc * clamped) / 100} ${circumference}`}
          transform="rotate(135 50 50)"
        />
      </svg>
      <div className="score-ring-text">
        <b className="oc-num">{number(clamped, lang, 0)}</b>
        <small>/ 100</small>
      </div>
    </div>
  );
}

/** A small score with its tone, for tables. */
export function ScorePill({ value, tone }: { value?: number | null; tone: string }) {
  const { lang, t } = useI18n();
  if (value === null || value === undefined) return <span className="faint">{t("unknownSignal")}</span>;
  return (
    <span className={`score-pill ${tone}`}>
      <span className="oc-dot" />
      <span className="oc-num">{number(value, lang, 0)}</span>
    </span>
  );
}

function paramText(value: unknown, lang: "tr" | "en"): string {
  if (Array.isArray(value)) return value.length ? value.join(", ") : "—";
  if (typeof value === "number") return number(value, lang, 2);
  if (typeof value === "boolean") return value ? "✓" : "✗";
  if (value === null || value === undefined) return "—";
  return String(value);
}

function ComponentRow({ component, total }: { component: ScoreComponent; total: number }) {
  const { t, lang } = useI18n();
  const { label, help } = reasonKeys(component);
  const known = component.normalized !== null && component.normalized !== undefined;
  const share = known && total > 0 ? Math.abs(component.contribution) / total : 0;
  const params = Object.entries(component.params ?? {}).filter(([, value]) => value !== null && !(Array.isArray(value) && value.length === 0));
  return (
    <li className={`breakdown-row ${known ? "" : "unknown"}`}>
      <div className="breakdown-head">
        <span className="breakdown-label">
          {label ? t(label) : component.id}
          {help && (
            <span className="breakdown-help" title={t(help)} aria-label={t(help)}>
              <Info size={12} />
            </span>
          )}
        </span>
        <span className={`breakdown-points oc-num ${component.contribution < 0 ? "negative" : ""}`}>
          {known || component.contribution !== 0
            ? `${component.contribution > 0 ? "+" : ""}${number(component.contribution, lang)}`
            : t("unknownSignal")}
        </span>
      </div>
      <div className="breakdown-bar" aria-hidden="true">
        <div style={{ width: `${Math.min(100, share * 100)}%` }} className={component.contribution < 0 ? "negative" : ""} />
      </div>
      {help && <p className="breakdown-text">{t(help)}</p>}
      {params.length > 0 && (
        <div className="breakdown-params">
          {params.map(([key, value]) => (
            <span key={key} className="oc-chip">
              <span>{hasKey(`param.${key}`) ? t(`param.${key}` as Key) : key.replace(/_/g, " ")}</span>
              {paramText(value, lang)}
            </span>
          ))}
        </div>
      )}
    </li>
  );
}

/** Every component of a score with what it adds, why, and the measured values. */
export function BreakdownList({ breakdown, compact = false }: { breakdown: Breakdown; compact?: boolean }) {
  const { t, lang } = useI18n();
  const components = ordered(breakdown);
  const total = components.reduce((sum, component) => sum + Math.abs(component.contribution), 0);
  return (
    <div className={`breakdown ${compact ? "compact" : ""}`}>
      <ul>
        {components.map((component) => (
          <ComponentRow key={component.id} component={component} total={total} />
        ))}
      </ul>
      <div className="breakdown-foot">
        <span>
          {t("confidence")} <b className="oc-num">{percent(breakdown.confidence, lang)}</b>
        </span>
        <span>
          {t("model")} <code className="oc-code">{breakdown.model}</code>
        </span>
      </div>
    </div>
  );
}
