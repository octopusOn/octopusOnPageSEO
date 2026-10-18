export const columnPreferencesKey = "spider.columns.v1";
export type ColumnPreferences = Record<string, string[]>;

/** Localized labels use the UI locale; stored field names keep ASCII case-insensitive matching. */
export function columnMatches(name: string, label: string, search: string, locale: string): boolean {
  const query = search.trim();
  return label.toLocaleLowerCase(locale).includes(query.toLocaleLowerCase(locale)) || name.toLowerCase().includes(query.toLowerCase());
}

/** Ignore malformed storage and unknown tabs; schema matching happens when results arrive. */
export function parseColumnPreferences(value: unknown, tabs: readonly string[]): ColumnPreferences {
  if (!value || typeof value !== "object" || Array.isArray(value)) return {};
  const saved = value as Record<string, unknown>;
  return Object.fromEntries(tabs.flatMap((tab) => {
    const columns = saved[tab];
    return Array.isArray(columns) && columns.length <= 256 && columns.every((name) => typeof name === "string" && name.length > 0 && name.length <= 128)
      ? [[tab, [...new Set(columns as string[])]]]
      : [];
  }));
}

/** Keep a valid saved choice, falling back if all of its fields disappeared in this schema. */
export function columnSelection(schema: readonly string[], defaults: readonly string[], saved: readonly string[] | undefined): string[] {
  const known = new Set(schema);
  const selected = saved?.filter((name) => known.has(name));
  return [...new Set(selected && (selected.length || saved?.length === 0) ? selected : defaults.filter((name) => known.has(name)))];
}

/** URL identity and scope-specific fields stay visible without changing the saved choice. */
export function visibleColumns(schema: readonly string[], defaults: readonly string[], saved: readonly string[] | undefined, required: readonly string[]): string[] {
  const fixed = required.filter((name) => schema.includes(name));
  return [...new Set([...fixed, ...columnSelection(schema, defaults, saved)])];
}

export function moveColumn(columns: readonly string[], name: string, direction: -1 | 1, required: readonly string[]): string[] {
  const next = [...columns];
  const index = next.indexOf(name), target = index + direction;
  if (index < 0 || target < 0 || target >= next.length || required.includes(name) || required.includes(next[target]!)) return next;
  [next[index], next[target]] = [next[target]!, next[index]!];
  return next;
}

/** Required fields injected for a temporary scope must not leak into the normal saved layout. */
export function saveColumnOrder(order: readonly string[], base: readonly string[], required: readonly string[]): string[] {
  return order.filter((name) => !required.includes(name) || base.includes(name));
}
