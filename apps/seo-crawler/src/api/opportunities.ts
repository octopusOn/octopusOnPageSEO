import {invoke} from "@tauri-apps/api/core";
import {open,save} from "@tauri-apps/plugin-dialog";
import {isNative} from "./client";
import type {TablePage} from "./types";

export type BusinessGoal="all-events"|"event"|"revenue";
export type OpportunityProject={id:string;name:string;site_url:string;gsc_property:string;ga4_property:string;google_connection:string;brand_terms:string[];goal:BusinessGoal;event_name:string;ga4_origin_confirmed:boolean;crawl_id:string|null};
export type DateRange={start:string;end:string};
export type AnalysisRequest={project_id:string;current:DateRange;previous:DateRange;country:string;device:string;brand:string;path_prefix:string};
export type Metrics={clicks:number;impressions:number;ctr:number|null;position:number|null};
export type Signal={kind:string;query:string|null;current:Metrics;previous:Metrics|null;benchmark_ctr:number|null;peer_urls:number;estimated_clicks:number|null;review_only:boolean};
export type GoalObservation={sessions:number;key_events:number|null;goal_rate:number|null;revenue:number|null;currency:string|null;boost:number};
export type QueryComparison={query:string;current:Metrics;previous:Metrics|null};
export type Opportunity={queries:QueryComparison[];url:string;current:Metrics;previous:Metrics|null;signals:Signal[];confidence:string;priority:number;priority_basis:string;impact_clicks:number|null;business:GoalObservation|null;actions:string[]};
export type DataCoverage={limited:boolean;warnings:string[];timezone:string;currency:string|null;google_organic:boolean;origin_confirmed:boolean;goal_key:string;dimensions:string[];site_url:string;property:string};
export type CoverageSummary={source:"gsc"|"ga4";grain:string;range:DateRange;rows:number;coverage:DataCoverage;origin:string;created_at_ms:number};
export type GoalTotals={sessions:number|null;key_events:number|null;revenue:number|null;currency:string|null};
export type Report={previous_ga4:GoalTotals|null;rule_version:string;request:AnalysisRequest;current:Metrics|null;previous:Metrics|null;sessions:number|null;key_events:number|null;revenue:number|null;currency:string|null;coverage:CoverageSummary[];warnings:string[];unknown_current_urls:number;daily:{date:string;metrics:Metrics}[]};
export type AnalysisSummary={id:string;total:number;report:Report;created_at_ms:number};
export type OpportunityPageRequest={analysis_id:string;offset:number;limit:number;search:string;kind:string;confidence:string};
export type OpportunityPage={rows:Opportunity[];total:number;offset:number};
export type WorkspaceStatus={projects:OpportunityProject[];bytes:number;datasets:number;rows:number;connections:{id:string;label:string}[];google:{configured:boolean;persistent_credentials:boolean;attempts:{id:string;status:string;error:string|null;connection_id:string|null}[]}};
export type GoogleCatalog={gsc:{id:string;name:string}[];ga4:{id:string;name:string}[];warnings:string[]};
export type SyncJob={id:string;project_id:string;kind:string;created_at_ms:number;analysis_id:string|null;status:string;progress:{phase:string;rows:number;requests:number;period:DateRange|null;retry_after_seconds:number|null};error:string|null};
export type PerformanceRow={url:string|null;query:string|null;date:string|null;country:string|null;device:string|null;clicks:number|null;impressions:number|null;position:number|null;sessions:number|null;key_events:number|null;goal_rate:number|null;revenue:number|null};
export type ImportRequest={project_id:string;path:string;sheet:string|null;source:"gsc"|"ga4";grain:string;range:DateRange;columns:Record<string,number>;locale:string;timezone:string;currency:string|null;google_organic:boolean;origin_confirmed:boolean};
export type ImportPreview={digest:string;sheets:string[];headers:string[];columns:Record<string,number>;rows:number;valid:number;invalid:number;duplicates:number;errors:{row:number;reason:string}[];samples:PerformanceRow[];missing:string[];warnings:string[]};
export type OpportunityDetail={opportunity:Opportunity;signal_total:number;query_total:number};
export type CrawlEvidence={crawl_id:string;crawl_name:string;created_at_ms:number;facts:TablePage;issue_ids:string[]};

function call<T>(command:string,args:Record<string,unknown>={}):Promise<T>{if(!isNative)return Promise.reject(new Error("SEO Opportunities is available in the desktop app. No Google data is fabricated in preview."));return invoke<T>(command,args);}
export const opportunitiesApi={
  status:()=>call<WorkspaceStatus>("opportunity_status"),
  saveProject:(project:OpportunityProject)=>call<OpportunityProject>("opportunity_project_save",{project}),
  removeProject:(id:string)=>call<void>("opportunity_project_remove",{id}),
  clearCache:(id:string)=>call<void>("opportunity_cache_clear",{id}),
  preview:(request:ImportRequest)=>call<ImportPreview>("opportunity_import_preview",{request}),
  import:(request:ImportRequest,digest:string)=>call<ImportPreview>("opportunity_import",{request,digest}),
  googleStart:()=>call<{id:string;url:string}>("opportunity_google_start"),
  importGoogleClient:(path:string)=>call<void>("opportunity_google_client_import",{path}),
  async chooseGoogleClient(){if(!isNative)return null;const value=await open({multiple:false,filters:[{name:"Google Desktop OAuth",extensions:["json"]}]});return typeof value==="string"?value:null;},
  googleCancel:(id:string)=>call<void>("opportunity_google_cancel",{id}),
  disconnect:(id:string)=>call<void>("opportunity_google_disconnect",{id}),
  catalog:(connection:string)=>call<GoogleCatalog>("opportunity_google_catalog",{connection}),
  sync:(request:AnalysisRequest)=>call<string>("opportunity_sync",{request}),
  jobs:()=>call<SyncJob[]>("opportunity_jobs"),
  cancel:(id:string)=>call<void>("opportunity_cancel",{id}),
  analyze:(request:AnalysisRequest)=>call<string>("opportunity_analyze",{request}),
  result:(id:string)=>call<AnalysisSummary>("opportunity_result",{id}),
  page:(request:OpportunityPageRequest)=>call<OpportunityPage>("opportunity_page",{request}),
  detail:(analysisId:string,url:string)=>call<OpportunityDetail>("opportunity_detail",{analysisId,url}),
  evidence:(id:string,url:string)=>call<CrawlEvidence>("opportunity_crawl_evidence",{id,url}),
  async chooseFile(){if(!isNative)return null;const value=await open({multiple:false,filters:[{name:"GSC / GA4",extensions:["csv","tsv","xlsx"]}]});return typeof value==="string"?value:null;},
  async export(request:OpportunityPageRequest,format:"csv"|"xlsx"){const path=await save({defaultPath:`seo-opportunities.${format}`,filters:[{name:format.toUpperCase(),extensions:[format]}]});return path?call<number>("opportunity_export",{request,path}):null;},
};
