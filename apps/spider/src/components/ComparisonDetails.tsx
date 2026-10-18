import {useState} from "react";
import type {Cell,TablePage} from "../api/types";
import {useI18n} from "../i18n";
import {comparisonDetailRows} from "../lib/comparison";
import {columnLabel} from "./DataTable";
import {Empty} from "./ui";

export function ComparisonDetails({data}:{data:TablePage}) {
  const {t}=useI18n();
  const [all,setAll]=useState(false);
  const rows=comparisonDetailRows(data,!all);
  const show=(value:Cell)=>value===null?<span className="faint">{t("compareNull")}</span>:value===""?<span className="faint">{t("compareEmpty")}</span>:String(value);
  if(!data.total) return <Empty>{t("noResults")}</Empty>;
  const baseline=data.rows[0]?.[data.columns.findIndex(c=>c.name==="in_baseline")]===true;
  const current=data.rows[0]?.[data.columns.findIndex(c=>c.name==="in_current")]===true;
  const membership=!baseline||!current;
  const shown=membership?comparisonDetailRows(data,false):rows;
  return <><div className="row between comparison-detail-intro"><p>{t(membership?"compareOneSide":"compareDetailNote")}</p>{!membership&&<label className="row"><input type="checkbox" checked={all} onChange={e=>setAll(e.target.checked)}/>{t("compareShowAll")}</label>}</div><table className="comparison-values"><thead><tr><th>{t("compareFieldFilter")}</th><th>{t("compareBaseline")}</th><th>{t("compareCurrent")}</th></tr></thead><tbody>{shown.map(row=><tr key={row.field} className={row.changed?"changed":""}><th>{columnLabel(row.field,t)}{row.changed&&<span className="compare-change-dot" aria-label={t("compareChanged")}/>}</th><td>{baseline?show(row.before):t("compareNotRecorded")}</td><td>{current?show(row.after):t("compareNotRecorded")}</td></tr>)}</tbody></table>{!shown.length&&<Empty>{t("compareNoFields")}</Empty>}</>;
}
