import {normalizeUrls} from "./config";

/** Local URL-list parsing. No network, filesystem or crawl side effects. */
export const URL_LIST_MAX_BYTES = 10 * 1024 * 1024;
export const URL_LIST_MAX_RECORDS = 100_000;
export type UrlListImportError = "size" | "records" | "format" | "csv" | "column" | "ambiguous" | "separator" | "encoding" | "read" | "existing" | "columns";
export class UrlListError extends Error {
  constructor(public readonly code: UrlListImportError) { super(code); }
}
export type InvalidUrlSample = { line: number; reason: "url" | "credentials" };
export type UrlListPreview = {
  urls: string[];
  records: number;
  duplicates: number;
  invalid: number;
  samples: InvalidUrlSample[];
};
type RecordRow = { cells: string[]; line: number };
const headers = new Set(["address", "url", "urls", "page", "landing page", "loc", "adres"]);

/** Strict HTTP URL normalization; fragments disappear, queries and paths retain their meaning. */
function importedUrl(raw: string): { url: string } | { reason: InvalidUrlSample["reason"] } {
  const value = raw.trim();
  if (!/^https?:\/\//i.test(value) || /\s|[\u0000-\u001f\u007f]/.test(value) || /%(?![\da-f]{2})/i.test(value)) return { reason: "url" };
  try {
    const url = new URL(value);
    if (!url.hostname || !["http:", "https:"].includes(url.protocol)) return { reason: "url" };
    if (url.username || url.password) return { reason: "credentials" };
    url.hash = "";
    return { url: url.href };
  } catch { return { reason: "url" }; }
}

/** Parses RFC-style quoted fields, including escaped quotes and embedded record newlines. */
function delimitedRows(text: string, separator: string): RecordRow[] {
  const rows: RecordRow[] = [];
  let cells: string[] = [], field = "", quoted = false, closed = false, line = 1, start = 1;
  const pushRecord = () => {
    cells.push(field);
    if (cells.some(cell => cell.trim() !== "")) {
      rows.push({ cells, line: start });
      if (rows.length > URL_LIST_MAX_RECORDS + 1) throw new UrlListError("records");
    }
    cells = []; field = ""; closed = false; start = line;
  };
  for (let i = 0; i < text.length; i++) {
    const char = text[i]!;
    if (quoted) {
      if (char === '"') {
        if (text[i + 1] === '"') { field += '"'; i++; }
        else { quoted = false; closed = true; }
      } else {
        field += char;
        if (char === "\n" || (char === "\r" && text[i + 1] !== "\n")) line++;
      }
      continue;
    }
    if (char === separator) {
      if(cells.length>=511)throw new UrlListError("columns");
      cells.push(field); field = ""; closed = false;
    }
    else if (char === "\n" || char === "\r") {
      if (char === "\r" && text[i + 1] === "\n") i++;
      line++; pushRecord();
    } else if (char === '"' && field === "" && !closed) quoted = true;
    else if (closed || char === '"') throw new UrlListError("csv");
    else field += char;
  }
  if (quoted) throw new UrlListError("csv");
  if (field !== "" || cells.length) pushRecord();
  return rows;
}

/** Read one logical record; a quoted header may span several physical lines. */
function firstDelimitedRecord(text: string): string {
  let quoted = false, start = 0;
  for (let i = 0; i < text.length; i++) {
    const char = text[i]!;
    if (char === '"') {
      if (quoted && text[i + 1] === '"') i++;
      else quoted = !quoted;
    } else if (!quoted && (char === "\n" || char === "\r")) {
      const record = text.slice(start, i);
      if (record.trim()) return record;
      if (char === "\r" && text[i + 1] === "\n") i++;
      start = i + 1;
    }
  }
  return text.slice(start);
}

function delimitedInput(text: string, extension: string): { text: string; separator: string; lineOffset: number } {
  const declaration = /^sep=([,;\t])(?:\r\n|\n|\r)/i.exec(text);
  if (declaration) return { text: text.slice(declaration[0].length), separator: declaration[1]!, lineOffset: 1 };
  if (extension === "tsv") return { text, separator: "\t", lineOffset: 0 };
  const first = firstDelimitedRecord(text);
  const separators = [",", ";", "\t"];
  // URL-header recognition takes precedence over punctuation in unrelated header names.
  const recognized = separators.filter(separator => {
    try {
      const cells = delimitedRows(first, separator)[0]?.cells;
      return !!cells && cells.length > 1 && cells.some(cell => headers.has(cell.trim().toLowerCase()));
    } catch (error) {
      if (!(error instanceof UrlListError)) throw error;
      return false;
    }
  });
  if (recognized.length > 1) throw new UrlListError("separator");
  if (recognized.length === 1) return { text, separator: recognized[0]!, lineOffset: 0 };
  // Headerless input still uses only punctuation outside quoted fields.
  let quoted = false;
  const counts = new Map(separators.map(separator => [separator, 0]));
  for (let i = 0; i < first.length; i++) {
    const char = first[i]!;
    if (char === '"') {
      if (quoted && first[i + 1] === '"') i++;
      else quoted = !quoted;
    } else if (!quoted && counts.has(char)) counts.set(char, counts.get(char)! + 1);
  }
  const separator = separators.sort((a, b) => counts.get(b)! - counts.get(a)!)[0]!;
  return { text, separator, lineOffset: 0 };
}

/** A file yields a preview only; malformed input never applies a partial list. */
export function parseUrlList(text: string, filename: string): UrlListPreview {
  if (new TextEncoder().encode(text).byteLength > URL_LIST_MAX_BYTES) throw new UrlListError("size");
  if (text.includes("\u0000")) throw new UrlListError("encoding");
  text = text.replace(/^\uFEFF/, "");
  const extension = filename.split(".").at(-1)?.toLowerCase();
  if (!["txt", "csv", "tsv"].includes(extension ?? "")) throw new UrlListError("format");
  let rows: RecordRow[];
  let column = 0;
  if (extension === "txt") {
    rows = [];
    const lines = text.split(/\r\n|\n|\r/);
    for (let index = 0; index < lines.length; index++) {
      const value = lines[index]!.trim();
      if (!value || value.startsWith("#")) continue;
      rows.push({ cells: [value], line: index + 1 });
      if (rows.length > URL_LIST_MAX_RECORDS) throw new UrlListError("records");
    }
  } else {
    const input = delimitedInput(text, extension!);
    rows = delimitedRows(input.text, input.separator).map(row => ({ ...row, line: row.line + input.lineOffset }));
    if (rows.length) {
      const width=rows[0]!.cells.length;
      if(rows.some(row=>row.cells.length!==width))throw new UrlListError("csv");
      const candidates = rows[0]!.cells.flatMap((cell, index) => headers.has(cell.trim().toLowerCase()) ? [index] : []);
      if (candidates.length > 1) throw new UrlListError("ambiguous");
      if (candidates.length) { column = candidates[0]!; rows.shift(); }
      else if (rows[0]!.cells.length !== 1) throw new UrlListError("column");
      if (rows.length > URL_LIST_MAX_RECORDS) throw new UrlListError("records");
    }
  }
  const result: UrlListPreview = { urls: [], records: rows.length, duplicates: 0, invalid: 0, samples: [] };
  const seen = new Set<string>();
  for (const row of rows) {
    const parsed = importedUrl(row.cells[column] ?? "");
    if ("reason" in parsed) {
      result.invalid++;
      if (result.samples.length < 5) result.samples.push({ line: row.line, reason: parsed.reason });
    } else if (seen.has(parsed.url)) result.duplicates++;
    else { seen.add(parsed.url); result.urls.push(parsed.url); }
  }
  return result;
}

/** Bounded UTF-8 file decoding. No requests are made for any imported address. */
export async function previewUrlList(file: Pick<File, "size" | "name" | "arrayBuffer">): Promise<UrlListPreview> {
  if (file.size > URL_LIST_MAX_BYTES) throw new UrlListError("size");
  let bytes: ArrayBuffer;
  try { bytes = await file.arrayBuffer(); } catch { throw new UrlListError("read"); }
  if (bytes.byteLength > URL_LIST_MAX_BYTES) throw new UrlListError("size");
  let text: string;
  try { text = new TextDecoder("utf-8", { fatal: true }).decode(bytes); } catch { throw new UrlListError("encoding"); }
  return parseUrlList(text, file.name);
}

/** Applies an explicitly reviewed import to a draft; failed append validation preserves the draft. */
export function mergeImportedUrls(existingText:string,imported:string[],append:boolean):string[] {
  if(!append)return [...imported];
  let existing:string[]=[];
  try {if(existingText.trim())existing=normalizeUrls(existingText,"list");} catch {throw new UrlListError("existing");}
  return [...new Set([...existing,...imported])];
}
