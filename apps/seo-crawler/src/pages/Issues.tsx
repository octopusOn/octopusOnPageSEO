import { useState } from "react";
import {
  Search,
  ArrowRight,
  EyeOff,
  CircleAlert,
  CheckCheck,
  X,
} from "lucide-react";
import type { Overview } from "../api/types";
import { categories, issueById, priorityOrder } from "../catalog";
import { useI18n, number } from "../i18n";
import { Button } from "../components/ui";

export function Issues({
  data,
  excluded,
  onExclude,
  onInspect,
}: {
  data: Overview;
  excluded: string[];
  onExclude: (id: string) => void;
  onInspect: (id: string) => void;
}) {
  const { t, lang } = useI18n();
  const [search, setSearch] = useState("");
  const [priority, setPriority] = useState("");
  const [category, setCategory] = useState("");
  const [showExcluded, setShowExcluded] = useState(false);
  const [selected, setSelected] = useState("");
  const items = data.issues
    .flatMap((count) => {
      const issue = issueById.get(count.id);
      return issue ? [{ ...count, issue }] : [];
    })
    .filter(
      (item) =>
        (showExcluded || !excluded.includes(item.id)) &&
        (!priority || item.issue.priority === priority) &&
        (!category || item.issue.category === category) &&
        item.issue[lang].title.toLowerCase().includes(search.toLowerCase()),
    )
    .sort(
      (a, b) =>
        (priorityOrder[a.issue.priority] ?? 2) -
          (priorityOrder[b.issue.priority] ?? 2) || b.count - a.count,
    );
  const current = items.find((i) => i.id === selected);
  const detail = current?.issue[lang];
  return (
    <div className="issues-layout">
      <div className="issues-main oc-panel">
        <div className="filter-toolbar">
          <div className="search-input">
            <Search size={16} />
            <input
              aria-label={t("searchIssues")}
              placeholder={t("searchIssues")}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>
          <select
            className="oc-select"
            aria-label={t("severity")}
            value={priority}
            onChange={(e) => setPriority(e.target.value)}
          >
            <option value="">
              {t("severity")}: {t("all")}
            </option>
            {(["high", "medium", "low"] as const).map((p) => (
              <option key={p} value={p}>
                {t(p)}
              </option>
            ))}
          </select>
          <select
            className="oc-select"
            aria-label={t("filter")}
            value={category}
            onChange={(e) => setCategory(e.target.value)}
          >
            <option value="">{t("all")}</option>
            {categories.map((c) => (
              <option key={c.id} value={c.id}>
                {c[lang]}
              </option>
            ))}
          </select>
        </div>
        <div className="issue-list-head">
          <span>{t("issue")}</span>
          <span>{t("severity")}</span>
          <span>{t("affected")}</span>
        </div>
        <div className="issue-table">
          {items.map((item) => (
            <button
              key={item.id}
              className={`issue-table-row ${selected === item.id ? "active" : ""} ${excluded.includes(item.id) ? "excluded" : ""}`}
              onClick={() => setSelected(item.id)}
            >
              <span className="row">
                <span className={`priority-mark ${item.issue.priority}`}>
                  <CircleAlert size={15} />
                </span>
                <span>
                  {item.issue[lang].title}
                  <small>
                    {
                      categories.find((c) => c.id === item.issue.category)?.[
                        lang
                      ]
                    }
                    {excluded.includes(item.id) && ` · ${t("excluded")}`}
                  </small>
                </span>
              </span>
              <span>
                <span
                  className={`oc-badge ${item.issue.priority === "high" ? "critical" : item.issue.priority === "medium" ? "warning" : "info"}`}
                >
                  {t(item.issue.priority as "high" | "medium" | "low")}
                </span>
              </span>
              <span className="oc-num">
                {number(item.count, lang)}
                <ArrowRight size={14} />
              </span>
            </button>
          ))}
          {!items.length && (
            <div className="sp-empty">
              <CheckCheck size={28} />
              {t("noIssues")}
            </div>
          )}
        </div>
        <footer className="issues-foot">
          <label className="oc-check">
            <input
              type="checkbox"
              checked={showExcluded}
              onChange={(e) => setShowExcluded(e.target.checked)}
            />
            {t("showExcluded")}
          </label>
          <span className="muted">
            {number(items.length, lang)} {t("checks")}
          </span>
        </footer>
      </div>
      <aside className="oc-panel issue-explanation">
        {current && detail ? (
          <>
            <header className="row between">
              <span className="eyebrow">{t("issueDetails")}</span>
              <Button
                className="icon ghost"
                aria-label={t("close")}
                onClick={() => setSelected("")}
              >
                <X size={16} />
              </Button>
            </header>
            <span className={`priority-mark large ${current.issue.priority}`}>
              <CircleAlert size={24} />
            </span>
            <h2>{detail.title}</h2>
            <span className="muted">
              {number(current.count, lang)} {t("affected")}
            </span>
            <h3>{t("why")}</h3>
            <p>{detail.description}</p>
            <h3>{t("fix")}</h3>
            <p>{detail.fix}</p>
            <Button primary onClick={() => onInspect(current.id)}>
              {t("inspectUrls")}
              <ArrowRight size={15} />
            </Button>
            <Button onClick={() => onExclude(current.id)}>
              <EyeOff size={15} />
              {t(excluded.includes(current.id) ? "include" : "exclude")}
            </Button>
            <small className="faint">{t("exclusionsNote")}</small>
          </>
        ) : (
          <div className="explanation-empty">
            <span className="oc-tile large teal">
              <ShieldIcon />
            </span>
            <h2>{t("technicalHealth")}</h2>
            <p>{t("healthNote")}</p>
          </div>
        )}
      </aside>
    </div>
  );
}
function ShieldIcon() {
  return <CheckCheck size={24} />;
}
