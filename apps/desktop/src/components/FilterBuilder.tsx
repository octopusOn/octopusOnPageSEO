// Simple query form: conditions as field / operator / value rows, optional grouping and
// sorting. It only writes query text; see `useFilterForm`.

import { useEffect, useMemo, useState } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { Plus, Trash2, X } from "lucide-react";

import { api } from "../api/commands";
import type { FieldKind, QuerySchema } from "../api/types";
import { isInteger, newRow, operatorsFor, type FilterForm, type FilterOp, type FilterRow } from "../lib/filterForm";
import { useI18n, type Translate } from "../i18n";
import type { MessageKey } from "../i18n/en";
import { keys } from "../state";
import { Button, Segmented } from "./ui";
import { ValuePicker, type Suggestion } from "./ValuePicker";

const CATEGORIES = ["search", "ai-training", "ai-search", "ai-assistant", "ads", "seo", "social", "monitoring", "other"];
const VERDICTS = ["verified", "spoofed", "unverified"];
const CONTENT_TYPES = ["html", "image", "js", "css", "font", "json", "xml", "pdf", "txt", "media", "document", "archive", "other"];
const REFERER_TYPES = ["direct", "internal", "search", "ai", "social", "email", "other"];
/** Fields with a matching grouping key, so their frequent values can be suggested. */
const SUGGESTABLE = new Set([
  "url", "path", "directory", "status", "method", "ip", "ua", "referer", "host", "protocol",
  "bot", "bot_category", "verification", "content_type", "referer_host", "referer_type",
  "country", "asn", "as_org",
]);
const MONOSPACE = new Set(["url", "path", "directory", "ua", "referer", "ip", "host", "referer_host"]);

/** Localized country name of an ISO code, e.g. `TR` → `Türkiye`. */
function countryName(code: string, locale: string): string | undefined {
  try {
    return new Intl.DisplayNames([locale], { type: "region" }).of(code);
  } catch {
    return undefined;
  }
}
const SUGGESTIONS = 50;
/** Columns of a hit listing (no group or metric). */
const LISTING_COLUMNS = ["ts", "ip", "method", "url", "status", "bytes", "response_ms", "ua", "bot", "verification", "referer", "host"];

function label(t: Translate, prefix: "field" | "metric" | "op", name: string): string {
  const key = `${prefix}.${name}` as MessageKey;
  const text = t(key);
  return text === key ? name : text;
}

interface Props {
  form: FilterForm | null;
  schema: QuerySchema | undefined;
  kinds: Record<string, FieldKind>;
  onChange: (form: FilterForm) => void;
  /** Clears the text so the form can be used again. */
  onReset: () => void;
  /** Show group, metric, sort and limit controls. */
  directives?: boolean;
  /** Site whose data feeds value suggestions. */
  site?: string;
  /** Conditions suggestions respect, e.g. the selected period (`date:last30d`). */
  context?: string | null;
}

export function FilterBuilder({ form, schema, kinds, onChange, onReset, directives = false, site, context }: Props) {
  const { t } = useI18n();
  if (!form) {
    return (
      <div className="builder">
        <div className="oc-banner info">
          <div className="grow">{t("builder.textOnly")}</div>
          <Button small onClick={onReset}>
            {t("builder.reset")}
          </Button>
        </div>
      </div>
    );
  }
  const fields = schema?.fields ?? [];
  const update = (index: number, row: FilterRow) =>
    onChange({ ...form, rows: form.rows.map((current, i) => (i === index ? row : current)) });
  const remove = (index: number) => onChange({ ...form, rows: form.rows.filter((_, i) => i !== index) });
  const add = () => {
    const field = form.rows.length === 0 ? "url" : "status";
    onChange({ ...form, rows: [...form.rows, newRow(field, kinds[field] ?? "text")] });
  };

  return (
    <div className="builder">
      {form.rows.length > 1 && (
        <div className="row small">
          <span className="muted">{t("builder.match")}</span>
          <Segmented<FilterForm["mode"]>
            value={form.mode}
            onChange={(mode) => onChange({ ...form, mode })}
            options={[
              { value: "and", label: t("builder.all") },
              { value: "or", label: t("builder.any") },
            ]}
          />
        </div>
      )}
      {form.rows.length === 0 && <p className="muted small">{t("builder.empty")}</p>}
      {form.rows.map((row, index) => (
        <ConditionRow
          // Rows have no identity beyond their position; that is enough for plain inputs.
          key={index}
          row={row}
          fields={fields}
          kind={kinds[row.field] ?? "text"}
          onChange={(next) => update(index, next)}
          onRemove={() => remove(index)}
          site={site}
          context={context ?? null}
        />
      ))}
      <div className="row">
        <Button small icon={<Plus size={14} />} onClick={add}>
          {t("builder.add")}
        </Button>
      </div>
      {directives && schema && <Directives form={form} schema={schema} onChange={onChange} />}
    </div>
  );
}

function ConditionRow({
  row,
  fields,
  kind,
  onChange,
  onRemove,
  site,
  context,
}: {
  row: FilterRow;
  fields: QuerySchema["fields"];
  kind: FieldKind;
  onChange: (row: FilterRow) => void;
  onRemove: () => void;
  site: string | undefined;
  context: string | null;
}) {
  const { t } = useI18n();
  const operators = operatorsFor(row.field, kind);
  const changeField = (field: string) => {
    const fieldKind = fields.find((f) => f.name === field)?.kind ?? "text";
    const next = newRow(field, fieldKind);
    // Keep the typed value when the new field accepts the same operator.
    onChange(operatorsFor(field, fieldKind).includes(row.op) && fieldKind === kind ? { ...row, field } : next);
  };
  return (
    <div className="builder-row">
      <select className="oc-select" value={row.field} onChange={(e) => changeField(e.target.value)}>
        {fields.map((field) => (
          <option key={field.name} value={field.name}>
            {label(t, "field", field.name)}
          </option>
        ))}
      </select>
      <select
        className="oc-select"
        value={row.op}
        onChange={(e) => onChange({ ...row, op: e.target.value as FilterOp, value: defaultValue(e.target.value as FilterOp, row.value) })}
      >
        {operators.map((op) => (
          <option key={op} value={op}>
            {label(t, "op", op)}
          </option>
        ))}
      </select>
      <ValueInput key={row.field} row={row} kind={kind} onChange={onChange} site={site} context={context} />
      <Button variant="ghost" small title={t("builder.remove")} icon={<Trash2 size={14} />} onClick={onRemove} />
    </div>
  );
}

function defaultValue(op: FilterOp, current: string): string {
  if (op === "is") return current === "false" ? "false" : "true";
  if (op === "last_days") return /^\d+$/.test(current) ? current : "30";
  if (op === "class") return /^[1-5]$/.test(current) ? current : "4";
  return current;
}

function ValueInput({
  row,
  kind,
  onChange,
  site,
  context,
}: {
  row: FilterRow;
  kind: FieldKind;
  onChange: (row: FilterRow) => void;
  site: string | undefined;
  context: string | null;
}) {
  const { t } = useI18n();
  const set = (patch: Partial<FilterRow>) => onChange({ ...row, ...patch });
  const suggest = useSuggestions(site, row.field, context);

  switch (row.op) {
    case "range": {
      const type = kind === "date" ? "date" : "number";
      return (
        <div className="builder-values">
          <input
            className="oc-input"
            type={type}
            min={0}
            value={row.min}
            placeholder={kind === "date" ? t("builder.from") : t("builder.min")}
            onChange={(e) => set({ min: e.target.value })}
          />
          <span className="muted">–</span>
          <input
            className="oc-input"
            type={type}
            min={0}
            value={row.max}
            placeholder={kind === "date" ? t("builder.to") : t("builder.max")}
            onChange={(e) => set({ max: e.target.value })}
          />
        </div>
      );
    }
    case "class":
      return (
        <select className="oc-select" value={row.value} onChange={(e) => set({ value: e.target.value })}>
          {["2", "3", "4", "5", "1"].map((c) => (
            <option key={c} value={c}>
              {c}xx
            </option>
          ))}
        </select>
      );
    case "last_days":
      return (
        <div className="builder-values">
          <input className="oc-input" type="number" min={1} value={row.value} onChange={(e) => set({ value: e.target.value })} />
          <span className="muted">{t("builder.days")}</span>
        </div>
      );
    case "is":
      return (
        <select className="oc-select" value={row.value} onChange={(e) => set({ value: e.target.value })}>
          <option value="true">{t("builder.yes")}</option>
          <option value="false">{t("builder.no")}</option>
        </select>
      );
    case "regex":
      return (
        <input
          className="oc-input grow mono"
          value={row.value}
          placeholder="^/(en|de)/"
          onChange={(e) => set({ value: e.target.value })}
        />
      );
    case "eq":
    case "neq":
      if (kind === "date") {
        return (
          <input className="oc-input" type="date" value={row.values[0] ?? ""} onChange={(e) => set({ values: [e.target.value] })} />
        );
      }
      return (
        <ValuePicker
          mode="multi"
          values={row.values}
          onChange={(values) => set({ values })}
          suggestions={suggest.suggestions}
          loading={suggest.loading}
          onSearch={suggest.setSearch}
          onOpen={suggest.open}
          isValid={kind === "integer" ? isInteger : undefined}
          freeTextHint={kind === "integer" ? t("picker.freeNumber") : undefined}
          monospace={MONOSPACE.has(row.field)}
        />
      );
    default:
      return (
        <ValuePicker
          mode="single"
          values={[row.value]}
          onChange={([value = ""]) => set({ value })}
          suggestions={suggest.suggestions}
          loading={suggest.loading}
          onSearch={suggest.setSearch}
          onOpen={suggest.open}
          monospace={MONOSPACE.has(row.field)}
          placeholder={row.op === "pattern" ? "/blog/*/amp" : undefined}
        />
      );
  }
}

/**
 * Frequent values of a field from the site's data, narrowed by typed text; bots, categories and
 * verdicts get readable labels and fall back to the known options when the data has none.
 */
function useSuggestions(site: string | undefined, field: string, context: string | null) {
  const { t, locale } = useI18n();
  const [search, setSearch] = useState("");
  const [debounced, setDebounced] = useState("");
  const [opened, setOpened] = useState(false);
  useEffect(() => {
    const handle = window.setTimeout(() => setDebounced(search.trim()), 200);
    return () => window.clearTimeout(handle);
  }, [search]);
  const enabled = opened && Boolean(site) && SUGGESTABLE.has(field);
  const data = useQuery({
    queryKey: ["site", site ?? "", "suggest", field, debounced, context ?? ""],
    queryFn: () => api.suggestValues(site ?? "", field, debounced, context, SUGGESTIONS),
    enabled,
    staleTime: 60_000,
    placeholderData: keepPreviousData,
    retry: false,
  });
  const bots = useQuery({ queryKey: keys.bots, queryFn: () => api.listBots(), staleTime: Infinity, enabled: opened && field === "bot" });

  const suggestions = useMemo<Suggestion[]>(() => {
    const found: Suggestion[] = (data.data ?? []).map((s) => ({ value: s.value, hits: s.hits }));
    const needle = debounced.toLowerCase();
    const known = (options: Suggestion[]) => {
      const missing = options.filter(
        (option) =>
          !found.some((f) => f.value === option.value) &&
          (!needle || `${option.value} ${option.label ?? ""}`.toLowerCase().includes(needle)),
      );
      return [...found, ...missing];
    };
    switch (field) {
      case "bot": {
        const names = new Map((bots.data ?? []).map((bot) => [bot.id, bot.name]));
        return known((bots.data ?? []).map((bot) => ({ value: bot.id }))).map((s) => ({ ...s, label: names.get(s.value) }));
      }
      case "bot_category":
        return known(CATEGORIES.map((value) => ({ value }))).map((s) => ({ ...s, label: t(`category.${s.value}` as MessageKey) }));
      case "verification":
        return known(VERDICTS.map((value) => ({ value }))).map((s) => ({ ...s, label: t(`verdict.${s.value}` as MessageKey) }));
      case "content_type":
        return known(CONTENT_TYPES.map((value) => ({ value }))).map((s) => ({ ...s, label: t(`contentType.${s.value}` as MessageKey) }));
      case "referer_type":
        return known(REFERER_TYPES.map((value) => ({ value }))).map((s) => ({ ...s, label: t(`refererType.${s.value}` as MessageKey) }));
      case "country":
        return found.map((s) => ({ ...s, label: countryName(s.value, locale) }));
      default:
        return found;
    }
  }, [data.data, bots.data, field, debounced, t, locale]);

  return {
    suggestions,
    loading: enabled && data.isFetching,
    setSearch,
    open: () => setOpened(true),
  };
}

function Directives({ form, schema, onChange }: { form: FilterForm; schema: QuerySchema; onChange: (form: FilterForm) => void }) {
  const { t } = useI18n();
  const aggregate = form.group.length > 0 || form.metrics.length > 0;
  const sortColumns = aggregate
    ? [...form.group, ...(form.metrics.length > 0 ? form.metrics : ["hits"])]
    : LISTING_COLUMNS;
  const sortLabel = (column: string) =>
    schema.metrics.includes(column) ? label(t, "metric", column) : label(t, "field", column === "ts" ? "date" : column);
  const first = form.sort[0];
  return (
    <div className="builder-directives">
      <MultiPick
        title={t("builder.group")}
        selected={form.group}
        options={schema.dimensions}
        label={(name) => label(t, "field", name)}
        onChange={(group) => onChange({ ...form, group, sort: keepSort(form.sort, group, form.metrics) })}
      />
      <MultiPick
        title={t("builder.metrics")}
        selected={form.metrics}
        options={schema.metrics}
        label={(name) => label(t, "metric", name)}
        onChange={(metrics) => onChange({ ...form, metrics, sort: keepSort(form.sort, form.group, metrics) })}
      />
      <div className="builder-option">
        <span>{t("builder.sort")}</span>
        <select
          className="oc-select"
          value={first?.column ?? ""}
          onChange={(e) =>
            onChange({
              ...form,
              sort: e.target.value
                ? [{ column: e.target.value, direction: first?.direction ?? "desc" }, ...form.sort.slice(1)]
                : [],
            })
          }
        >
          <option value="">{t("builder.none")}</option>
          {sortColumns.map((column) => (
            <option key={column} value={column}>
              {sortLabel(column)}
            </option>
          ))}
        </select>
        {first && (
          <select
            className="oc-select"
            value={first.direction}
            onChange={(e) =>
              onChange({ ...form, sort: [{ ...first, direction: e.target.value as "asc" | "desc" }, ...form.sort.slice(1)] })
            }
          >
            <option value="desc">{t("builder.desc")}</option>
            <option value="asc">{t("builder.asc")}</option>
          </select>
        )}
      </div>
      <div className="builder-option">
        <span>{t("builder.limit")}</span>
        <input
          className="oc-input"
          type="number"
          min={1}
          style={{ width: 110 }}
          value={form.limit}
          placeholder={t("builder.none")}
          onChange={(e) => onChange({ ...form, limit: e.target.value })}
        />
      </div>
    </div>
  );
}

/** Sort keys must refer to result columns; drop keys whose column disappeared. */
function keepSort(sort: FilterForm["sort"], group: string[], metrics: string[]): FilterForm["sort"] {
  const aggregate = group.length > 0 || metrics.length > 0;
  const columns = aggregate ? [...group, ...(metrics.length > 0 ? metrics : ["hits"])] : LISTING_COLUMNS;
  return sort.filter((key) => columns.includes(key.column));
}

function MultiPick({
  title,
  selected,
  options,
  label,
  onChange,
}: {
  title: string;
  selected: string[];
  options: string[];
  label: (name: string) => string;
  onChange: (selected: string[]) => void;
}) {
  const { t } = useI18n();
  const remaining = options.filter((option) => !selected.includes(option));
  return (
    <div className="builder-option">
      <span>{title}</span>
      <div className="oc-chips">
        {selected.map((name) => (
          <span className="oc-chip" key={name}>
            <span>{label(name)}</span>
            <button type="button" onClick={() => onChange(selected.filter((s) => s !== name))}>
              <X size={12} />
            </button>
          </span>
        ))}
        {remaining.length > 0 && (
          <select
            className="oc-select small-select"
            value=""
            onChange={(e) => e.target.value && onChange([...selected, e.target.value])}
          >
            <option value="">{t("builder.addOption")}</option>
            {remaining.map((name) => (
              <option key={name} value={name}>
                {label(name)}
              </option>
            ))}
          </select>
        )}
      </div>
    </div>
  );
}
