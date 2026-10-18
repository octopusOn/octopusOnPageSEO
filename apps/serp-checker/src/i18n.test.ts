import { expect, it } from "vitest";
import { fileName } from "./api/client";
import { hasKey } from "./i18n";

// The scoring models live in Rust; their reason keys and parameters are read from the source.
const sources = import.meta.glob<string>("../../../crates/octopuson-serp-score/src/*.rs", {
  query: "?raw",
  import: "default",
  eager: true,
});
const rust = Object.entries(sources)
  .filter(([path]) => !path.endsWith("/tests.rs"))
  .map(([, text]) => text)
  .join("\n");

it("explains every reason the scoring models report", () => {
  const reasons = new Set((rust.match(/"(?:kd|content|technical|site|opportunity)\.[a-z_.]+"/g) ?? []).map((text) => text.slice(1, -1)));
  expect(reasons.size).toBeGreaterThan(20);
  for (const reason of reasons) expect(hasKey(`reason.${reason}`), reason).toBe(true);
});

it("names every measured value the scoring models report", () => {
  const params = new Set([...rust.matchAll(/\.with\(\s*"([a-z_]+)"/g)].map((match) => match[1]!));
  expect(params.size).toBeGreaterThan(20);
  for (const param of params) expect(hasKey(`param.${param}`), param).toBe(true);
});

it("makes safe file names from keywords", () => {
  expect(fileName(" türk kahvesi: nasıl/yapılır? ")).toBe("türk-kahvesi-nasıl-yapılır");
  expect(fileName("   ")).toBe("export");
});
