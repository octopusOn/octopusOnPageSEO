import type { SeoScore } from "../api/types";
import { categories, issues, issueById } from "../catalog";

/** Demo weighting; native v3 additionally verifies stored canonical/hreflang context. */
export function demoSeoScore(urls:{rules:string[];analyzed:boolean;limited:boolean}[]):SeoScore {
  const totals=new Map<string,{count:number;points:number}>();
  const categoryLoss=new Map<string,{loss:number;affected:number}>();
  let loss=0, affected=0, unknown=0, advisory=0;
  const ignored=new Map<string,number>();
  for(const url of urls) {
    const effective=[...new Set(url.rules)].filter(id=>{if(["hreflang-missing-x-default","links-too-many"].includes(id)){advisory++;ignored.set(id,(ignored.get(id)??0)+1);return false;}return true;});
    const rules=effective.map(id=>({id,priority:issueById.get(id)?.priority}));
    const known=rules.filter(rule=>rule.priority).map(rule=>({...rule,weight:({high:20,medium:7,low:2} as Record<string,number>)[rule.priority!]!}));
    unknown+=rules.length-known.length;
    if(known.length)affected++;
    const raw=known.reduce((sum,rule)=>sum+rule.weight,0),capped=Math.min(100,raw);
    loss+=capped;
    const grouped=new Map<string,number>();
    for(const rule of known){const category=issueById.get(rule.id)!.category;grouped.set(category,(grouped.get(category)??0)+rule.weight);}
    for(const [id,raw] of grouped){const previous=categoryLoss.get(id)??{loss:0,affected:0};categoryLoss.set(id,{loss:previous.loss+Math.min(100,raw),affected:previous.affected+1});}
    for(const rule of known) {
      const previous=totals.get(rule.id)??{count:0,points:0};
      totals.set(rule.id,{count:previous.count+1,points:previous.points+rule.weight*capped/raw/urls.length});
    }
  }
  const analyzed=urls.filter(url=>url.analyzed).length;
  const deductions=[...totals].map(([id,row])=>({id,...row})).sort((a,b)=>b.points-a.points||a.id.localeCompare(b.id));
  return {version:4,value:urls.length&&analyzed&&!unknown ? Math.round((100-loss/urls.length)*10)/10 : null,
    urls:urls.length,affected_urls:affected,analyzed_html:analyzed,limited_urls:urls.filter(url=>url.limited).length,unknown_findings:unknown,
    deductions,excluded_urls:0,excluded_links:0,excluded_findings:advisory,advisory_findings:advisory,excluded_rules:[...ignored].map(([id,count])=>({id,count})),
    categories:categories.map(category=>{
      const rows=deductions.filter(row=>issueById.get(row.id)?.category===category.id),measured=categoryLoss.get(category.id)??{loss:0,affected:0};
      const coverage=["response-codes","security","url"].includes(category.id)?urls.length:["robots","ai","rendering","pagination"].includes(category.id)?0:analyzed;
      const count=Math.max(coverage,measured.affected);
      return {id:category.id,value:count&&!unknown?Math.round(Math.max(0,100-measured.loss/count)*10)/10:null,points:rows.reduce((sum,row)=>sum+row.points,0),urls:count,affected_urls:measured.affected,checks:issues.filter(rule=>rule.category===category.id).length,deductions:rows};
    })};
}
