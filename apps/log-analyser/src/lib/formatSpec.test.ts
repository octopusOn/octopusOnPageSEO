import { describe, expect, it } from "vitest";

import { describeSpec, formatInputFromSpec } from "./formatSpec";

describe("format specs", () => {
  it("parse every stored kind", () => {
    expect(formatInputFromSpec("auto")).toEqual({ kind: "auto" });
    expect(formatInputFromSpec("builtin:traefik-clf")).toEqual({ kind: "builtin", id: "traefik-clf" });
    expect(formatInputFromSpec('nginx:$remote_addr - [$time_local] "$request"')).toEqual({
      kind: "nginx",
      format: '$remote_addr - [$time_local] "$request"',
    });
    expect(describeSpec("auto", "auto-detect")).toBe("auto-detect");
  });
});
