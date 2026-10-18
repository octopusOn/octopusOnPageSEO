import { useState } from "react";
import { ArrowDown, ArrowUp, LockKeyhole, RotateCcw, Search } from "lucide-react";
import { useI18n, number } from "../i18n";
import { columnLabel } from "./DataTable";
import { Button, Modal } from "./ui";
import { columnMatches, columnSelection, moveColumn, saveColumnOrder, visibleColumns } from "../lib/columns";

export function ColumnSettings({ tabLabel, schema, defaults, saved, required, onApply, onClose }: {
  tabLabel: string;
  schema: string[];
  defaults: string[];
  saved: string[] | undefined;
  required: string[];
  onApply: (columns: string[] | undefined) => void;
  onClose: () => void;
}) {
  const { t, lang } = useI18n();
  const [draft, setDraft] = useState(() => saved && [...saved]);
  const [search, setSearch] = useState("");
  const base = columnSelection(schema, defaults, draft);
  const visible = visibleColumns(schema, defaults, draft, required);
  const available = schema.filter((name) => !visible.includes(name));
  const matches = (name: string) => columnMatches(name, columnLabel(name,t), search, lang);
  function move(name: string, direction: -1 | 1) {
    setDraft(saveColumnOrder(moveColumn(visible, name, direction, required), base, required));
  }
  return <Modal title={`${tabLabel} · ${t("tableColumns")}`} className="column-modal" onClose={onClose} footer={<>
    <Button onClick={onClose}>{t("cancel")}</Button>
    <Button primary onClick={() => onApply(draft === undefined ? undefined : base)}>{t("applyColumns")}</Button>
  </>}>
    <div className="column-settings">
      <p className="column-settings-note">{t("columnsNote")}</p>
      <div className="row column-settings-actions">
        <Button small onClick={() => setDraft([...visible, ...available])}>{t("selectAllColumns")}</Button>
        <Button small onClick={() => setDraft(base.filter((name) => required.includes(name)))}>{t("clearOptionalColumns")}</Button>
        <Button small onClick={() => setDraft(undefined)}><RotateCcw size={13}/>{t("resetColumns")}</Button>
        <span className="muted">{number(visible.length, lang)} / {number(schema.length, lang)}</span>
      </div>
      <div className="search-input column-search"><Search size={15}/><input value={search} onChange={(e) => setSearch(e.target.value)} placeholder={t("searchColumns")} aria-label={t("searchColumns")}/></div>
      <div className="column-settings-grid">
        <section aria-label={t("visibleColumns")}><h3>{t("visibleColumns")} <small>{number(visible.length, lang)}</small></h3><div className="column-settings-list">
          {visible.filter(matches).map((name) => {
            const label = columnLabel(name, t), fixed = required.includes(name), index = visible.indexOf(name);
            return <div className="column-option" key={name}>
              <label className="oc-check"><input type="checkbox" checked disabled={fixed} onChange={() => setDraft(base.filter((column) => column !== name))}/><span>{label}</span></label>
              {fixed ? <span className="column-fixed" title={t("requiredColumn")} aria-label={t("requiredColumn")}><LockKeyhole size={12}/></span> : <span className="row column-order">
                <Button small className="icon ghost" disabled={index === 0 || required.includes(visible[index-1]!)} aria-label={`${t("moveColumnUp")}: ${label}`} onClick={() => move(name,-1)}><ArrowUp size={13}/></Button>
                <Button small className="icon ghost" disabled={index === visible.length-1} aria-label={`${t("moveColumnDown")}: ${label}`} onClick={() => move(name,1)}><ArrowDown size={13}/></Button>
              </span>}
            </div>;
          })}
        </div></section>
        <section aria-label={t("availableColumns")}><h3>{t("availableColumns")} <small>{number(available.length, lang)}</small></h3><div className="column-settings-list">
          {available.filter(matches).map((name) => <div className="column-option" key={name}><label className="oc-check"><input type="checkbox" checked={false} onChange={() => setDraft([...base,name])}/><span>{columnLabel(name,t)}</span></label></div>)}
        </div></section>
      </div>
      {!schema.some(matches) && <p className="muted">{t("noResults")}</p>}
      <p className="column-settings-note"><LockKeyhole size={11}/> {t("requiredColumnsNote")}</p>
    </div>
  </Modal>;
}
