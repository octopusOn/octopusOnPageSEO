import { expect, it } from "vitest";
import { defaultConfig } from "../api/types";
import { validConfig } from "./config";
import { desktopViewport, effectiveViewport, mobileViewport, validViewport, viewportPreset } from "./viewport";

it("preserves automatic legacy layouts and lets an explicit viewport override only layout", () => {
  expect(effectiveViewport(defaultConfig)).toEqual(desktopViewport);
  const smartphone = { ...defaultConfig, agent: "googlebot-smartphone" };
  expect(effectiveViewport(smartphone)).toEqual({ width: 412, height: 915, mobile: true });
  const custom = { ...smartphone, rendering: { ...smartphone.rendering, viewport: { width: 1440, height: 900, mobile: false } } };
  expect(effectiveViewport(custom)).toEqual(custom.rendering.viewport);
  expect(custom.agent).toBe("googlebot-smartphone");
  expect(viewportPreset(undefined)).toBe("auto");
});

it("rejects invalid dimensions and mobile flags through the same start validation as the UI", () => {
  for (const size of [0, 239, 3841, -1, 390.5, NaN, Infinity]) {
    for (const viewport of [{ width: size, height: 800, mobile: true }, { width: 1280, height: size, mobile: false }]) {
      expect(validViewport(viewport)).toBe(false);
      expect(validConfig({ ...defaultConfig, rendering: { ...defaultConfig.rendering, viewport } })).toBe(false);
    }
  }
  for (const size of [240, 390, 1280, 3840]) expect(validViewport({ width: size, height: size, mobile: false })).toBe(true);
  expect(validViewport({width:390,height:844,mobile:undefined} as never)).toBe(false);
  expect(validViewport(null)).toBe(true);
  expect(validConfig(defaultConfig)).toBe(true);
});

it("distinguishes mobile and desktop layouts even at identical dimensions", () => {
  expect(viewportPreset(desktopViewport)).toBe("desktop");
  expect(viewportPreset(mobileViewport)).toBe("mobile");
  expect(viewportPreset({ ...mobileViewport, mobile: false })).toBe("custom");
  expect(viewportPreset({ width: 768, height: 1024, mobile: true })).toBe("custom");
});
