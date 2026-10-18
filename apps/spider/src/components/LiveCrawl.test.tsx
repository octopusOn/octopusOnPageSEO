import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { defaultConfig, type Job, type UrlActivity } from "../api/types";
import { LiveCrawl } from "./LiveCrawl";

const entry: UrlActivity = {
  id: 1, url: "https://example.com/dynamic", kind: "page", state: "error",
  started_ms: 0, elapsed_ms: 1200, status: 200,
  title: "A <script>title</script>", error: null, rendered: false,
  render_error: "Browser timeout <script>error</script>", response_ms: 50, size_bytes: 1024,
};
const job: Job = {
  id: "job-1", url: "https://example.com/", status: "running", phase: "crawling",
  rendering: "javascript", revision: 2,
  progress: { crawled: 1, queued: 2, in_flight: 1, discovered: 4, errors: 0, elapsed_ms: 2000, sitemaps: 0, paused: false },
  activity: {
    active: [{ ...entry, id: 2, url: "https://example.com/processing", state: "processing", status: null, render_error: null }],
    recent: [entry], rendered: 0, render_errors: 1, request_errors: 0, blocked: 0,
  },
};
const render = (value: Job) => renderToStaticMarkup(<LiveCrawl running={value} onControl={async () => {}} />);

describe("live crawl screen", () => {
  it("shows current URLs, HTTP 200 with a JS error and safely escapes remote diagnostics", () => {
    const html = render(job);
    expect(html).toContain("https://example.com/processing");
    expect(html).toContain("https://example.com/dynamic");
    expect(html).toContain("JS hatası · Ham HTML");
    expect(html).toContain(">200</span>");
    expect(html).toContain("&lt;script&gt;error&lt;/script&gt;");
    expect(html).not.toContain("<script>");
  });
  it("disables pause and stop while saving and shows an indeterminate progress bar", () => {
    const html = render({ ...job, phase: "finalizing", activity: { ...job.activity!, active: [] } });
    expect(html).toContain("Analiz ediliyor ve kaydediliyor");
    expect(html.match(/<button[^>]*disabled/g)).toHaveLength(2);
    expect(html).not.toContain("aria-valuenow");
  });
  it("explains missing activity from older native versions", () => {
    expect(render({ ...job, activity: undefined })).toContain("Bu arka uç URL akışını göndermiyor");
  });
  it("shows the matching crawl's limits and HTTP timing without inventing settings when missing", () => {
    const html = renderToStaticMarkup(<LiveCrawl running={job} config={{ ...defaultConfig, speed: { threads: 3, max_requests_per_second: 0 }, rendering: { ...defaultConfig.rendering, mode: "javascript", concurrency: 8 }, urls: { ...defaultConfig.urls, start_folder_only: true } }} onControl={async () => {}} />);
    expect(html).toContain("Eşzamanlı tarayıcı sayfası</dt><dd>3");
    expect(html).toContain("Yalnızca başlangıç klasörleri");
    expect(html).toContain("Sınırsız");
    expect(html).toContain("50 ms");
    expect(html).toContain("1 başarılı sayfa yanıtı");
    const legacy = render({ ...job, activity: undefined });
    expect(legacy).not.toContain("Başlangıç klasörleriyle sınırlı değil");
    expect(legacy).not.toContain("Eşzamanlı tarayıcı sayfası");
    expect(legacy).toContain("süre verileri bekleniyor");
  });
  it("shows HTML mode and wider folder coverage without claiming every URL is included", () => {
    const html = renderToStaticMarkup(<LiveCrawl running={{ ...job, rendering: "html" }} config={defaultConfig} onControl={async () => {}} />);
    expect(html).toContain("Ham HTML");
    expect(html).toContain("Başlangıç klasörleriyle sınırlı değil");
    expect(html).toContain("başlangıç adresinin klasörü dışındaki bağlantılar");
    expect(html).not.toContain("Eşzamanlı tarayıcı sayfası");
  });
});
