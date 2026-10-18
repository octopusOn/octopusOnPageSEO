import type { ScoreCategory } from "../api/types";
import { number, type Key, type Lang } from "../i18n";

export const seoSystems:{id:string;label:Key;categories:string[]}[]=[
  {id:"content",label:"seoSystemContent",categories:["titles","meta-descriptions","headings","content"]},
  {id:"access",label:"seoSystemAccess",categories:["response-codes","canonicals","directives","sitemaps","robots"]},
  {id:"architecture",label:"seoSystemArchitecture",categories:["url","links","pagination"]},
  {id:"experience",label:"seoSystemExperience",categories:["performance","mobile"]},
  {id:"security",label:"seoSystemSecurity",categories:["security"]},
  {id:"assets",label:"seoSystemAssets",categories:["images"]},
  {id:"international",label:"seoSystemInternational",categories:["international"]},
  {id:"structure",label:"seoSystemStructure",categories:["structured-data","social","ai"]},
  {id:"rendering",label:"seoSystemRendering",categories:["rendering"]},
];

export function categoryTone(value:number|null):string {
  return value===null ? "unknown" : value>=90 ? "excellent" : value>=75 ? "good" : value>=50 ? "improve" : "poor";
}

export function systemPoints(system:typeof seoSystems[number],categories:ScoreCategory[]):number {
  return categories.filter(category=>system.categories.includes(category.id)).reduce((sum,category)=>sum+category.points,0);
}

/** Keep small positive deductions visible instead of rounding them to zero. */
export function scorePointText(points:number,lang:Lang):string {
  return points>0 && points<0.1 ? `<${number(0.1,lang)}` : number(Math.max(0,points),lang);
}

export function scoreLossText(points:number,lang:Lang):string {
  return points>0 ? `−${scorePointText(points,lang)}` : "0";
}
