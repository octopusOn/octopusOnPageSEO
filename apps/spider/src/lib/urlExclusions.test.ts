import { expect, it } from "vitest";
import { defaultConfig } from "../api/types";

it("keeps Cloudflare defaults editable in the existing exclusion list with strict path boundaries", () => {
  const patterns=defaultConfig.urls.exclude.map(pattern=>new RegExp(pattern));
  expect(patterns).toHaveLength(4);
  const excluded=(path:string)=>patterns.some(pattern=>pattern.test(`https://example.com${path}`));
  for(const path of ["/cdn-cgi/l/email-protection#abc", "/cdn-cgi/scripts/5c5dd728/cloudflare-static/email-decode.min.js", "/cdn-cgi/challenge-platform/h/g/", "/cdn-cgi/trace", "/cdn-cgi/rum?ray=1", "/cdn-cgi/script_monitor/report"])expect(excluded(path)).toBe(true);
  for(const path of ["/cdn-cgi/image/width=800/photo.jpg", "/cdn-cgi/l/email-protection-guide", "/cdn-cgi/challenge-platform-guide", "/photo.jpg?next=/cdn-cgi/rum"])expect(excluded(path)).toBe(false);
  const edited=structuredClone(defaultConfig); edited.urls.exclude=[];
  expect(edited.urls.exclude.some(pattern=>new RegExp(pattern).test("https://example.com/cdn-cgi/l/email-protection"))).toBe(false);
  expect(defaultConfig.urls.exclude).toHaveLength(4);
});
