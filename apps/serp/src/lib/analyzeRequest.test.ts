import { expect, it } from "vitest";
import { requestOf } from "./analyzeRequest";

const form = {
  keyword: "  türk kahvesi ",
  engine: "yandex" as const,
  country: "TR",
  language: "Tr",
  device: "mobile" as const,
  target: "",
  targetUrl: " ",
  pages: 20,
  volume: true,
  backlinks: false,
  javascript: true,
};

it("sends a trimmed query and leaves optional fields out", () => {
  const request = requestOf(form);
  expect(request.search.query).toEqual({ engine: "yandex", keyword: "türk kahvesi", country: "tr", language: "tr", device: "mobile", depth: 10 });
  expect(request.search.target).toBeUndefined();
  expect(request.target_url).toBeUndefined();
  expect(request.pages).toBe(20);
  expect(request.crawl).toEqual({ javascript: true, identity: "octopus" });
});

it("compares the user's site by registrable domain", () => {
  const request = requestOf({ ...form, target: "https://www.Örnek.com.tr/", targetUrl: "https://ornek.com.tr/kahve" });
  expect(request.search.target).toEqual({ value: "https://www.Örnek.com.tr/", mode: "domain" });
  expect(request.target_url).toBe("https://ornek.com.tr/kahve");
});
