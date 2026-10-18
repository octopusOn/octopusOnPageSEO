import { expect, it } from "vitest";
import { demoSeoScore } from "./demoSeoScore";
import { seoActionPlan } from "./seoActionPlan";

it("prioritizes severity before score impact, then affected coverage and a stable tie break", () => {
  const score = demoSeoScore([{rules:["anchor-empty"],analyzed:true,limited:false}]);
  score.deductions = [
    {id:"title-too-wide",count:10,points:5},
    {id:"missing-hsts",count:100,points:10},
    {id:"uncompressed",count:8,points:5},
    {id:"anchor-empty",count:10,points:5},
    {id:"hreflang-missing-return",count:3,points:.4},
    {id:"slow-response",count:5,points:8},
  ];
  const expected = ["hreflang-missing-return","slow-response","anchor-empty","title-too-wide","uncompressed","missing-hsts"];
  expect(seoActionPlan(score).map(action=>action.rule.id)).toEqual(expected);
  expect(seoActionPlan(score).map(action=>action.rank)).toEqual([1,2,3,4,5,6]);
  score.deductions.reverse();
  expect(seoActionPlan(score).map(action=>action.rule.id)).toEqual(expected);
});

it("omits ignored-only, unknown and unmeasured deductions while preserving mixed real evidence", () => {
  const score = demoSeoScore([{rules:["anchor-empty"],analyzed:true,limited:false}]);
  score.excluded_rules = [{id:"links-to-broken",count:260},{id:"anchor-empty",count:20}];
  score.deductions = [
    {id:"anchor-empty",count:2,points:.5},
    {id:"uncompressed",count:30,points:0},
    {id:"internal-client-error",count:0,points:3},
    {id:"title-too-wide",count:5,points:NaN},
    {id:"slow-response",count:5,points:Infinity},
    {id:"future-check",count:10,points:20},
  ];
  const actions = seoActionPlan(score);
  expect(actions.map(action=>action.rule.id)).toEqual(["anchor-empty"]);
  expect(actions[0]!.deduction).toEqual({id:"anchor-empty",count:2,points:.5});
  expect(actions[0]!.rule.tr.fix).not.toBe("");
  expect(actions[0]!.rule.en.fix).not.toBe("");
  score.value = null;
  expect(seoActionPlan(score)).toEqual([]);
  expect(seoActionPlan()).toEqual([]);
  expect(seoActionPlan({...score,version:99,value:80})).toEqual([]);
});
