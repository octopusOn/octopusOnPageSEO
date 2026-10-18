import type {SyncJob} from "../api/opportunities";
import {number,useI18n} from "../i18n";
import {opportunityPhase} from "../lib/opportunities";
import {Button} from "./ui";

export function OpportunityJobProgress({job,onCancel}:{job:SyncJob;onCancel:()=>void}){
 const {lang}=useI18n();const tr=lang==="tr";const progress=job.progress;
 return <div className="op-sync" role="status" aria-live="polite"><span className="loading-dot"/><span>{opportunityPhase(progress.phase,tr)}{progress.period?` · ${progress.period.start} → ${progress.period.end}`:""} · {number(progress.rows,lang)} {tr?"alınan satır":"rows received"} · {number(progress.requests,lang)} {tr?"istek":"requests"}{progress.retry_after_seconds!=null?` · ${tr?"Google kota/geçici hatası, yeniden deneme":"Google quota/temporary error, retry"}: ${progress.retry_after_seconds} ${tr?"sn":"s"}`:""}</span><Button small disabled={progress.phase==="cancelling"} onClick={onCancel}>{tr?"İşlemi iptal et":"Cancel operation"}</Button></div>;
}
