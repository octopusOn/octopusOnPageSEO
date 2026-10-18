import { expect, it } from "vitest";
import { issues } from "../catalog";
import { demoSeoScore } from "./demoSeoScore";

it("matches native v2 weighting, distinct findings, per-URL caps and total contributions",()=>{
  const high=issues.filter(issue=>issue.priority==="high").slice(0,7).map(issue=>issue.id);
  const medium=issues.find(issue=>issue.priority==="medium")!.id,low=issues.find(issue=>issue.priority==="low")!.id;
  const score=demoSeoScore([{rules:[high[0]!,high[0]!,medium],analyzed:true,limited:false},{rules:[low],analyzed:true,limited:false}]);
  expect(score.categories!.reduce((sum,row)=>sum+row.points,0)).toBeCloseTo(14.5);
  expect(score.value).toBe(85.5);expect(score.deductions.reduce((sum,row)=>sum+row.points,0)).toBe(14.5);
  const capped=demoSeoScore([{rules:high,analyzed:true,limited:false},{rules:[],analyzed:true,limited:false}]);
  expect(capped.value).toBe(50);expect(capped.deductions.reduce((sum,row)=>sum+row.points,0)).toBeCloseTo(50);
});

it("distinguishes unavailable samples and unknown rules from a clean score",()=>{
  expect(demoSeoScore([]).value).toBeNull();
  expect(demoSeoScore([{rules:[],analyzed:false,limited:true}]).value).toBeNull();
  expect(demoSeoScore([{rules:["future-rule"],analyzed:true,limited:false}]).value).toBeNull();
  expect(demoSeoScore([{rules:[],analyzed:true,limited:false}]).value).toBe(100);
});

it("keeps optional default-language and link-density advice outside score penalties",()=>{
 const score=demoSeoScore([{rules:["hreflang-missing-x-default","links-too-many"],analyzed:true,limited:false}]);
 expect(score.version).toBe(4);expect(score.value).toBe(100);expect(score.deductions).toEqual([]);expect(score.advisory_findings).toBe(2);
});
