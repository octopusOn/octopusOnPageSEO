// The interface texts: one JSON file per language in apps/portal/messages. A new language is a
// new file (with every key of the others) plus the language in the CMS; no code changes.

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

/** Groups of plain texts can also be read by a key from the data: `t.billing.orderStatus[status]`. */
type Texts<T> = {
  [K in keyof T]: T[K] extends string ? string : T[K] extends Record<string, string> ? T[K] & Record<string, string> : Texts<T[K]>;
};

/** The shape of a messages file; tr.json is the reference the others are checked against. */
export type Messages = Texts<typeof import("../../messages/tr.json")>;

const FILE = /^([a-z]{2,3}(?:-[a-z0-9]{2,8})?)\.json$/;

let cache: Map<string, Messages> | undefined;

/** Every messages file by language code, read once per process (on each call in development). */
export function catalog(directory = join(process.cwd(), "messages")): Map<string, Messages> {
  if (cache && process.env.NODE_ENV === "production") return cache;
  const files = readdirSync(directory)
    .map((name) => FILE.exec(name)?.[1])
    .filter((code): code is string => Boolean(code))
    .sort();
  cache = new Map(files.map((code) => [code, JSON.parse(readFileSync(join(directory, `${code}.json`), "utf8")) as Messages]));
  return cache;
}

/** The texts of `code`, with any key it lacks taken from `fallback`. */
export function messagesFor(code: string, fallback: string): Messages {
  const all = catalog();
  const own = all.get(code);
  const base = all.get(fallback) ?? all.values().next().value;
  if (!own) return base as Messages;
  return base && base !== own ? (merge(base, own) as Messages) : own;
}

function merge(base: unknown, own: unknown): unknown {
  if (!isRecord(base) || !isRecord(own)) return own ?? base;
  const out: Record<string, unknown> = { ...base };
  for (const [key, value] of Object.entries(own)) out[key] = merge(base[key], value);
  return out;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
