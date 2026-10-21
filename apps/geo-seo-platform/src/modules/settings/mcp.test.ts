import { describe, expect, it } from "vitest";
import { claudeCodeCommand, desktopConfig, isoOf, mcpUrl, webConfig } from "./mcp";

describe("assistant settings", () => {
  it("starts the desktop command over standard input and output", () => {
    expect(JSON.parse(desktopConfig())).toEqual({ mcpServers: { "octopuson-platform": { command: "octopuson-platform", args: ["mcp"] } } });
    expect(claudeCodeCommand(null, "x")).toBe("claude mcp add octopuson-platform -- octopuson-platform mcp");
    const app = "/Applications/OctopusOn GEO-SEO Platform.app/Contents/MacOS/octopuson-platform-desktop";
    expect(JSON.parse(desktopConfig(app)).mcpServers["octopuson-platform"].command).toBe(app);
    expect(claudeCodeCommand(null, "", app)).toBe(`claude mcp add octopuson-platform -- '${app}' mcp`);
    expect(claudeCodeCommand(null, "", "C:\\Program Files\\it's\\app.exe")).toBe("claude mcp add octopuson-platform -- 'C:\\Program Files\\it'\\''s\\app.exe' mcp");
  });

  it("points web assistants at the address with the token", () => {
    const url = mcpUrl("https://app.octopuson.com/");
    expect(url).toBe("https://app.octopuson.com/mcp");
    expect(JSON.parse(webConfig(url, "opt_abc")).mcpServers["octopuson-platform"]).toEqual({
      type: "http",
      url,
      headers: { Authorization: "Bearer opt_abc" },
    });
    expect(claudeCodeCommand(url, "opt_abc")).toBe('claude mcp add --transport http octopuson-platform https://app.octopuson.com/mcp --header "Authorization: Bearer opt_abc"');
  });

  it("reads Unix seconds", () => {
    expect(isoOf(1_790_000_000)).toBe("2026-09-21T14:13:20.000Z");
  });
});
