import { describe, expect, it, vi } from "vitest";
import { choose, demoCloud } from "./demoCloud";
import type { ApiConfig } from "./types";

const noKeys: ApiConfig = {
  google_api_key: "",
  google_cx: "",
  yandex_api_key: "",
  yandex_user: "",
  serper_api_key: "",
  serpapi_key: "",
};

describe("OctopusOn SEO credits in the preview", () => {
  it("prefers the own key in automatic mode, like the desktop app", () => {
    expect(choose("auto", true, true)).toBe("own");
    expect(choose("auto", false, true)).toBe("credit");
    expect(choose("auto", false, false)).toBeNull();
    expect(choose("own", false, true)).toBeNull();
    expect(choose("credit", true, true)).toBe("credit");
  });

  it("offers Serper and SerpApi on credits once connected, never Google or Yandex XML", () => {
    vi.useFakeTimers();
    try {
      const before = demoCloud.sources(noKeys);
      expect(before.find((s) => s.id === "serper")?.via).toBeNull();

      const login = demoCloud.startLogin();
      expect(login.status).toBe("waiting");
      vi.advanceTimersByTime(2000);
      expect(demoCloud.loginState()?.status).toBe("approved");
      expect(demoCloud.status().signed_in).toBe(true);

      const sources = demoCloud.sources({ ...noKeys, serpapi_key: "own" });
      expect(sources.find((s) => s.id === "serper")?.via).toBe("credit");
      expect(sources.find((s) => s.id === "serpapi")?.via).toBe("own");
      const google = sources.find((s) => s.id === "google-cse");
      expect(google?.creditable).toBe(false);
      expect(google?.credit).toBeNull();
      expect(() => demoCloud.setMode("google-cse", "credit")).toThrow();

      demoCloud.setMode("serpapi", "credit");
      expect(
        demoCloud.sources({ ...noKeys, serpapi_key: "own" }).find((s) => s.id === "serpapi")?.via,
      ).toBe("credit");
      demoCloud.setMode("serpapi", "auto");
      demoCloud.logout();
      expect(demoCloud.sources(noKeys).find((s) => s.id === "serper")?.via).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});
