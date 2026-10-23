// The filters of a list report: presets, the filters in force as chips, a form to add one and
// the order. Whether they run here (every row loaded, free) or at DataForSEO (every row it
// holds, priced first) is the page's choice; the bar says which.

import { useEffect, useRef, useState, type FormEvent } from "react";
import { ArrowDownWideNarrow, Filter as FilterIcon, Plus, X } from "lucide-react";
import { Button } from "@octopusseo/ui";
import { useI18n } from "../i18n";
import { formatNumber } from "./format";
import { MAX_FILTERS, OPS, parseValue, withFilter, type FilterField, type ListQuery, type QueryFilter, type QueryOp, type QueryOrder } from "./query";
import { featureName } from "./tables";

/** Result-page elements a keyword list can be filtered by. */
export const FEATURES = [
  "ai_overview",
  "featured_snippet",
  "people_also_ask",
  "local_pack",
  "images",
  "video",
  "short_videos",
  "top_stories",
  "knowledge_graph",
  "shopping",
  "discussions_and_forums",
  "related_searches",
  "paid",
] as const;

const INTENTS = ["informational", "navigational", "commercial", "transactional"] as const;

/** A ready filter set, toggled with one click. */
export interface FilterPreset {
  id: string;
  label: string;
  filters: QueryFilter[];
}

export type FilterScope = "local" | "server";

function sameFilter(left: QueryFilter, right: QueryFilter): boolean {
  return left.field === right.field && left.op === right.op && JSON.stringify(left.value) === JSON.stringify(right.value);
}

/** Whether every filter of a preset is in force. */
export function presetOn(preset: FilterPreset, filters: readonly QueryFilter[]): boolean {
  return preset.filters.every((wanted) => filters.some((filter) => sameFilter(filter, wanted)));
}

/** A preset switched on or off among the filters in force. */
export function togglePreset(preset: FilterPreset, filters: readonly QueryFilter[]): QueryFilter[] {
  if (presetOn(preset, filters)) return filters.filter((filter) => !preset.filters.some((wanted) => sameFilter(filter, wanted)));
  return preset.filters.reduce<QueryFilter[]>((all, next) => withFilter(all, next), [...filters]);
}

const OP_SIGNS: Partial<Record<QueryOp, string>> = { eq: "=", ne: "≠", lt: "<", le: "≤", gt: ">", ge: "≥" };

function useFilterText() {
  const { t, locale } = useI18n();
  const opName = (op: QueryOp) => OP_SIGNS[op] ?? t(`query.op_${op}`);
  const valueText = (field: FilterField, value: QueryFilter["value"]) => {
    if (field.kind === "flag") return value ? t("query.yes") : t("query.no");
    if (field.kind === "list") return featureName(String(value), t);
    if (field.name === "intent" && typeof value === "string") return t(`overview.intent_${value}`);
    if (typeof value === "number") return formatNumber(value, locale);
    return Array.isArray(value) ? value.join(", ") : String(value);
  };
  const filterText = (fields: FilterField[], filter: QueryFilter) => {
    const field = fields.find((entry) => entry.name === filter.field);
    if (!field) return `${filter.field} ${filter.op} ${String(filter.value)}`;
    if (field.kind === "flag") return `${t(field.label)}: ${valueText(field, filter.value)}`;
    return `${t(field.label)} ${opName(filter.op)} ${valueText(field, filter.value)}`;
  };
  return { opName, filterText };
}

function ValueInput({ field, op, text, onText }: { field: FilterField; op: QueryOp; text: string; onText: (text: string) => void }) {
  const { t } = useI18n();
  if (field.kind === "flag")
    return (
      <select className="oc-select" value={text || "true"} onChange={(event) => onText(event.target.value)} aria-label={t("query.value")}>
        <option value="true">{t("query.yes")}</option>
        <option value="false">{t("query.no")}</option>
      </select>
    );
  if (field.kind === "list")
    return (
      <select className="oc-select" value={text} onChange={(event) => onText(event.target.value)} aria-label={t("query.value")}>
        <option value="">{t("query.choose")}</option>
        {FEATURES.map((feature) => (
          <option key={feature} value={feature}>
            {featureName(feature, t)}
          </option>
        ))}
      </select>
    );
  if (field.name === "intent" && (op === "eq" || op === "ne"))
    return (
      <select className="oc-select" value={text} onChange={(event) => onText(event.target.value)} aria-label={t("query.value")}>
        <option value="">{t("query.choose")}</option>
        {INTENTS.map((intent) => (
          <option key={intent} value={intent}>
            {t(`overview.intent_${intent}`)}
          </option>
        ))}
      </select>
    );
  const hint = op === "in" || op === "not_in" ? t("query.listHint") : op === "regex" || op === "not_regex" ? t("query.regexHint") : undefined;
  return (
    <input
      className={`oc-input${field.kind === "number" ? " narrow" : ""}`}
      inputMode={field.kind === "number" ? "decimal" : undefined}
      value={text}
      placeholder={hint}
      onChange={(event) => onText(event.target.value)}
      aria-label={t("query.value")}
      autoFocus
    />
  );
}

function AddFilter({ fields, onAdd, onClose }: { fields: FilterField[]; onAdd: (filter: QueryFilter) => void; onClose: () => void }) {
  const { t } = useI18n();
  const { opName } = useFilterText();
  const [name, setName] = useState(fields[0]?.name ?? "");
  const field = fields.find((entry) => entry.name === name) ?? fields[0];
  const [op, setOp] = useState<QueryOp>(field ? OPS[field.kind][0]! : "eq");
  const [text, setText] = useState("");
  const box = useRef<HTMLFormElement>(null);
  useEffect(() => {
    const away = (event: MouseEvent) => {
      if (!box.current?.contains(event.target as Node)) onClose();
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("mousedown", away);
      document.removeEventListener("keydown", key);
    };
  }, [onClose]);
  if (!field) return null;
  const value = parseValue(field.kind, op, field.kind === "flag" ? text || "true" : text);
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (value === null) return;
    onAdd({ field: field.name, op, value });
  };
  return (
    <form className="oc-menu filter-form" ref={box} onSubmit={submit} aria-label={t("query.add")}>
      <select
        className="oc-select"
        value={field.name}
        aria-label={t("query.field")}
        onChange={(event) => {
          const next = fields.find((entry) => entry.name === event.target.value);
          setName(event.target.value);
          if (next) setOp(OPS[next.kind][0]!);
          setText("");
        }}
      >
        {fields.map((entry) => (
          <option key={entry.name} value={entry.name}>
            {t(entry.label)}
          </option>
        ))}
      </select>
      {OPS[field.kind].length > 1 && (
        <select className="oc-select" value={op} aria-label={t("query.operator")} onChange={(event) => setOp(event.target.value as QueryOp)}>
          {OPS[field.kind].map((choice) => (
            <option key={choice} value={choice}>
              {opName(choice)}
            </option>
          ))}
        </select>
      )}
      <ValueInput field={field} op={op} text={text} onText={setText} />
      <Button primary small type="submit" disabled={value === null}>
        {t("query.addButton")}
      </Button>
    </form>
  );
}

export function FilterBar({
  fields,
  query,
  onChange,
  scope,
  presets = [],
  total,
}: {
  fields: FilterField[];
  /** The filters and order in force (the offset is the pager's). */
  query: Pick<ListQuery, "filters" | "order">;
  onChange: (next: Pick<ListQuery, "filters" | "order">) => void;
  /** Where the next change runs. */
  scope: FilterScope;
  presets?: FilterPreset[];
  /** Rows DataForSEO holds for the question, when known. */
  total?: number;
}) {
  const { t, locale } = useI18n();
  const { filterText } = useFilterText();
  const [adding, setAdding] = useState(false);
  const sortable = fields.filter((field) => field.kind === "number" || field.kind === "text");
  const order = query.order[0];
  const orderValue = order ? `${order.field}:${order.direction}` : "";
  const setOrder = (value: string) => {
    const [field, direction] = value.split(":");
    const next: QueryOrder[] = field && (direction === "asc" || direction === "desc") ? [{ field, direction }] : [];
    onChange({ ...query, order: next });
  };
  const full = query.filters.length >= MAX_FILTERS;
  const active = query.filters.length > 0 || query.order.length > 0;
  return (
    <div className="filter-bar" role="group" aria-label={t("query.filters")}>
      <div className="filter-bar-row">
        <span className="filter-bar-label">
          <FilterIcon size={14} aria-hidden="true" />
          {t("query.filters")}
        </span>
        {presets.map((preset) => {
          const on = presetOn(preset, query.filters);
          return (
            <button key={preset.id} type="button" className={`oc-chip toggle${on ? " on" : ""}`} aria-pressed={on} onClick={() => onChange({ ...query, filters: togglePreset(preset, query.filters) })}>
              {preset.label}
            </button>
          );
        })}
        {query.filters
          .filter((filter) => !presets.some((preset) => presetOn(preset, query.filters) && preset.filters.some((wanted) => sameFilter(wanted, filter))))
          .map((filter) => (
            <span key={`${filter.field}|${filter.op}|${JSON.stringify(filter.value)}`} className="oc-chip filter-chip">
              {filterText(fields, filter)}
              <button type="button" className="chip-remove" aria-label={t("query.remove", { filter: filterText(fields, filter) })} onClick={() => onChange({ ...query, filters: query.filters.filter((entry) => entry !== filter) })}>
                <X size={12} aria-hidden="true" />
              </button>
            </span>
          ))}
        <span className="filter-add">
          <Button small className="ghost" disabled={full} onClick={() => setAdding(!adding)} aria-expanded={adding} title={full ? t("query.full", { n: MAX_FILTERS }) : undefined}>
            <Plus size={14} aria-hidden="true" />
            {t("query.add")}
          </Button>
          {adding && (
            <AddFilter
              fields={fields}
              onClose={() => setAdding(false)}
              onAdd={(filter) => {
                setAdding(false);
                onChange({ ...query, filters: withFilter(query.filters, filter) });
              }}
            />
          )}
        </span>
        <label className="filter-order">
          <ArrowDownWideNarrow size={14} aria-hidden="true" />
          <span className="sr-only">{t("query.order")}</span>
          <select className="oc-select" value={orderValue} onChange={(event) => setOrder(event.target.value)} aria-label={t("query.order")}>
            <option value="">{t("query.orderDefault")}</option>
            {sortable.map((field) =>
              (["desc", "asc"] as const).map((direction) => (
                <option key={`${field.name}:${direction}`} value={`${field.name}:${direction}`}>
                  {t(field.label)} {direction === "desc" ? "↓" : "↑"}
                </option>
              )),
            )}
          </select>
        </label>
        {active && (
          <Button small className="ghost" onClick={() => onChange({ filters: [], order: [] })}>
            {t("query.clear")}
          </Button>
        )}
      </div>
      <p className={`filter-scope ${scope}`}>
        {scope === "local"
          ? t("query.scopeLocal")
          : total !== undefined
            ? t("query.scopeServerTotal", { total: formatNumber(total, locale) })
            : t("query.scopeServer")}
      </p>
    </div>
  );
}
