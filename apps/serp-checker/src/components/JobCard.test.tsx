import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import type { Job } from "../api/types";
import { JobCard } from "./JobCard";

const base: Job = {
  id: "job-1",
  kind: "analysis",
  label: "türk kahvesi",
  engine: "google",
  status: "running",
  step: "crawl",
  done: 4,
  total: 10,
  started_at: 0,
  revision: 3,
};
const render = (job: Job) => renderToStaticMarkup(<JobCard job={job} onStop={() => {}} />);

it("shows crawl progress with a stop button", () => {
  const html = render(base);
  expect(html).toContain("Sıralanan sayfalar taranıyor · 4/10");
  expect(html).toContain("width:40%");
  expect(html).toContain("Durdur");
});

it("asks the person to clear a check page and never claims to solve it", () => {
  const html = render({
    ...base,
    status: "challenge",
    step: "page",
    detail: "browser:1/2",
    challenge: { provider: "browser", kind: "captcha", url: "https://www.google.com/sorry/index" },
  });
  expect(html).toContain("doğrulama sayfası");
  expect(html).toContain("sizin yerinize çözmez");
  expect(html).toContain("/sorry/");
});

it("lists the sources tried when a search fails", () => {
  const html = render({
    ...base,
    status: "failed",
    error: "no provider returned results",
    attempts: [
      { provider: "serper", outcome: "skipped", message: "no API key" },
      { provider: "browser", outcome: { failed: "blocked" } },
    ],
  });
  expect(html).toContain('role="alert"');
  expect(html).toContain("serper");
  expect(html).toContain("atlandı · no API key");
  expect(html).toContain("blocked");
});
