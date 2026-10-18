import {renderToStaticMarkup} from "react-dom/server";
import {expect,it} from "vitest";
import {OpportunityOverview} from "./OpportunityReport";
import type {AnalysisSummary} from "../api/opportunities";
import {newAnalysis} from "../lib/opportunities";
const summary:AnalysisSummary={id:"test",total:1,created_at_ms:1792310000000,report:{rule_version:"test",request:newAnalysis("test"),current:{clicks:10,impressions:1000,ctr:0.01,position:8},previous:null,sessions:null,key_events:null,revenue:null,currency:null,warnings:["missing-rows-not-zero","ga4-unavailable"],coverage:[],previous_ga4:null,unknown_current_urls:1,daily:[]}};
it("renders absent GA4 as unknown and explains observed coverage",()=>{const html=renderToStaticMarkup(<OpportunityOverview summary={summary}/>);expect(html).toContain("GA4 · Google organik oturum");expect(html).toContain("—");expect(html).toContain("Sıfır trafik veya kesin kayıp sayılmadı");expect(html).toContain("iş değeri katkısı tarafsız");expect(html).not.toContain("op-trend-bars");});
it("escapes unknown coverage notices instead of rendering remote HTML",()=>{const html=renderToStaticMarkup(<OpportunityOverview summary={{...summary,report:{...summary.report,warnings:['<script>alert(1)</script>']}}}/>);expect(html).not.toContain("<script>");expect(html).toContain("&lt;script&gt;");});
