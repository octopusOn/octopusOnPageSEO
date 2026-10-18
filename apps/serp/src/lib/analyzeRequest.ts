import type { AnalyzeRequest, Device, Engine } from "../api/types";

/** The analysis form as typed. */
export interface Form {
  keyword: string;
  engine: Engine;
  country: string;
  language: string;
  device: Device;
  target: string;
  targetUrl: string;
  pages: number;
  volume: boolean;
  backlinks: boolean;
  javascript: boolean;
}

/** The request for an analysis of what the form holds. */
export function requestOf(form: Form): AnalyzeRequest {
  const target = form.target.trim();
  return {
    search: {
      query: {
        engine: form.engine,
        keyword: form.keyword.trim(),
        country: form.country.trim().toLowerCase(),
        language: form.language.trim().toLowerCase(),
        device: form.device,
        depth: 10,
      },
      ...(target ? { target: { value: target, mode: "domain" as const } } : {}),
    },
    pages: form.pages,
    crawl: { javascript: form.javascript, identity: "octopus" },
    ...(form.targetUrl.trim() ? { target_url: form.targetUrl.trim() } : {}),
    volume: form.volume,
    backlinks: form.backlinks,
  };
}
