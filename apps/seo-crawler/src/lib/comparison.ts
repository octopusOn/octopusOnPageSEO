import type { CrawlMeta, Job, ComparisonRequest, TablePage } from "../api/types";
import type {Key} from "../i18n";
import { isActiveJob } from "./jobs";

export function comparisonCandidates(crawls:CrawlMeta[],jobs:Job[]) {
  const active=new Set(jobs.filter(isActiveJob).map(job=>job.crawl_id));
  return crawls.filter(crawl=>crawl.status!=="running" && !active.has(crawl.id));
}

export class ComparisonPreparationError extends Error {}

/** Explicitly open each saved analysis; comparison itself remains read-only. */
export async function prepareComparisonArchives(
  baseline:CrawlMeta,
  current:CrawlMeta,
  readJobs:()=>Promise<Job[]>,
  open:(id:string)=>Promise<unknown>,
) {
  if(!baseline.id||!current.id||baseline.id===current.id||baseline.status==="running"||current.status==="running")
    throw new ComparisonPreparationError();
  // A fresh native snapshot also catches finalization after metadata was saved.
  const jobs=await readJobs();
  if(jobs.some(job=>isActiveJob(job)&&(job.crawl_id===baseline.id||job.crawl_id===current.id)))
    throw new ComparisonPreparationError();
  // Keep migrations sequential and stop immediately if either archive cannot be opened.
  await open(baseline.id);
  await open(current.id);
}

// A complete identity includes both archives, scope and every filter. Never export a stale page.
export function comparisonIdentity(baseline:string,current:string,request:ComparisonRequest) {
  return JSON.stringify([baseline,current,request]);
}

export function comparisonDetailRows(data:TablePage,onlyChanged:boolean) {
  const field=data.columns.findIndex(c=>c.name==="field"), before=data.columns.findIndex(c=>c.name==="before_value"),after=data.columns.findIndex(c=>c.name==="after_value"), changed=data.columns.findIndex(c=>c.name==="changed");
  return data.rows.filter(row=>!onlyChanged || row[changed]===true).map(row=>({field:String(row[field]??""),before:row[before]??null,after:row[after]??null,changed:row[changed]===true}));
}

export function comparisonSummaryRequest(request:ComparisonRequest):ComparisonRequest {return {...request,filter:"all",offset:0,limit:250};}
export const comparisonWarningKeys:Record<string,Key>={"partial":"comparePartial","seeds-differ":"compareSeeds","settings-differ":"compareSettings","list-mode":"compareList","incomplete-content":"compareIncomplete","no-shared-urls":"compareNoShared"};
