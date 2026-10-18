import type { Cell, TablePage } from "../api/types";
import { number, useI18n, type Key } from "../i18n";
import { Empty } from "./ui";

export function RenderingComparison({ facts }: { facts: TablePage }) {
  const { t, lang } = useI18n();
  const value = (name: string): Cell | undefined => facts.rows[0]?.[facts.columns.findIndex((column) => column.name === name)];
  const rendered = value("rendered");
  if (typeof rendered !== "boolean") return <Empty>{t("renderCompareMissing")}</Empty>;
  const baseline = typeof value("raw_noindex") === "boolean";
  const truncated = value("render_truncated") === true;
  const rows: { label: Key | "noindex" | "nofollow"; before: Cell | undefined; after: Cell | undefined; known: boolean }[] = [
    { label: "address", before: value("url"), after: value("rendered_url"), known: true },
    { label: "title", before: value("raw_title"), after: value("title"), known: true },
    { label: "meta_description", before: value("raw_meta_description"), after: value("meta_description"), known: baseline },
    { label: "canonical", before: value("raw_canonical"), after: value("canonical") ?? value("header_canonical"), known: baseline },
    { label: "noindex", before: value("raw_noindex"), after: value("noindex"), known: baseline },
    { label: "nofollow", before: value("raw_nofollow"), after: value("nofollow"), known: baseline },
    { label: "word_count", before: value("raw_word_count"), after: value("word_count"), known: true },
    { label: "outlinks", before: value("raw_link_count"), after: value("rendered_link_count"), known: true },
  ];
  const display = (cell: Cell | undefined) => typeof cell === "boolean" ? t(cell ? "yes" : "no") : typeof cell === "number" ? number(cell, lang) : cell ?? "—";
  const removed = rendered && !truncated && value("raw_noindex") === true && value("noindex") === false;
  return <div className="render-comparison">
    <p className="render-comparison-note">{t("renderCompareHeaders")}</p>
    {!baseline && <p className="render-comparison-note">{t("renderCompareLegacy")}</p>}
    {!rendered && <p className="render-comparison-warning">{t("renderCompareFailed")} {display(value("render_error"))}</p>}
    {truncated && <p className="render-comparison-warning">{t("renderCompareTruncated")}</p>}
    {removed && <p className="render-comparison-warning">{t("renderCompareNoindexRemoved")}</p>}
    <table>
      <thead><tr><th>{t("renderCompareField")}</th><th>{t("renderCompareBefore")}</th><th>{t("renderCompareAfter")}</th><th>{t("renderCompareChange")}</th></tr></thead>
      <tbody>{rows.map((row) => {
        const comparable = row.known && rendered;
        const changed = comparable && (row.before ?? null) !== (row.after ?? null);
        return <tr key={row.label}>
          <th scope="row">{row.label === "noindex" || row.label === "nofollow" ? row.label : t(row.label)}</th>
          <td>{row.known ? display(row.before) : <span className="muted">{t("renderCompareUnknown")}</span>}</td>
          <td>{rendered ? display(row.after) : <span className="muted">{t("renderCompareUnavailable")}</span>}</td>
          <td><span className={changed ? "render-changed" : "muted"}>{comparable ? t(changed ? "renderCompareChanged" : "renderCompareSame") : "—"}</span></td>
        </tr>;
      })}</tbody>
    </table>
  </div>;
}
