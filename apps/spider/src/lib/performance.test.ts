import { expect, it } from "vitest";
import { defaultConfig } from "../api/types";
import { validConfig } from "./config";
import { activeSpeedProfile, applySpeedProfile, effectiveBrowserConcurrency, performanceNumber, speedProfiles } from "./performance";

it("changes the three JavaScript speed limits without changing the crawl's coverage or settling time",()=>{
  const original={...structuredClone(defaultConfig),robots:"ignore-but-report",storage:{headers:false,html:true},rendering:{...defaultConfig.rendering,mode:"javascript" as const,wait_ms:50,timeout_seconds:1,cache_resources:false,viewport:{width:390,height:844,mobile:true}}};
  const before=structuredClone(original);
  for(const profile of Object.keys(speedProfiles) as (keyof typeof speedProfiles)[]) {
    const next=applySpeedProfile(original,profile);
    expect(validConfig(next)).toBe(true);
    expect(activeSpeedProfile(next)).toBe(profile);
    expect(next.rendering.wait_ms).toBe(50);
    expect({...next,speed:original.speed,rendering:{...next.rendering,concurrency:original.rendering.concurrency}}).toEqual(original);
  }
  expect(original).toEqual(before);
});

it("retains hidden browser settings when an HTML-only crawl uses a speed profile",()=>{
  const original={...defaultConfig,rendering:{...defaultConfig.rendering,concurrency:7}};
  const next=applySpeedProfile(original,"fast");
  expect(next.speed).toEqual({threads:10,max_requests_per_second:20});
  expect(next.rendering).toEqual(original.rendering);
  expect(activeSpeedProfile(next)).toBe("fast");
});

it("identifies custom fractional/unlimited rates and applies both limits to the browser bound",()=>{
  for(const rate of [0,0.5,Number.MIN_VALUE,99.5]) {
    const config={...defaultConfig,speed:{threads:3,max_requests_per_second:rate},rendering:{...defaultConfig.rendering,mode:"javascript" as const,concurrency:8}};
    expect(activeSpeedProfile(config)).toBeNull();
    expect(effectiveBrowserConcurrency(config)).toBe(3);
    expect(validConfig(config)).toBe(true);
  }
  expect(effectiveBrowserConcurrency({...defaultConfig,speed:{...defaultConfig.speed,threads:64},rendering:{...defaultConfig.rendering,concurrency:1}})).toBe(1);
  for(const threads of [0,65,NaN,1.5])expect(effectiveBrowserConcurrency({...defaultConfig,speed:{...defaultConfig.speed,threads}})).toBeNull();
});

it("rejects a cleared or invalid speed value while preserving explicit unlimited and fractional input",()=>{
  for(const input of [""," ","NaN","Infinity","-1"])expect(validConfig({...defaultConfig,speed:{...defaultConfig.speed,max_requests_per_second:performanceNumber(input)}})).toBe(false);
  for(const input of ["0","0.25","5e-324","20"])expect(performanceNumber(input)).toBe(Number(input));
});
