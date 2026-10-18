import type { CrawlConfig, Viewport } from "../api/types";

export const desktopViewport: Viewport = { width: 1280, height: 800, mobile: false };
export const mobileViewport: Viewport = { width: 390, height: 844, mobile: true };
export type ViewportPreset = "auto" | "desktop" | "mobile" | "custom";

export function validViewport(viewport: Viewport | null | undefined) {
  return viewport == null || (typeof viewport === "object" &&
    [viewport.width, viewport.height].every(value => Number.isInteger(value) && value >= 240 && value <= 3840) &&
    typeof viewport.mobile === "boolean");
}

export function viewportPreset(viewport: Viewport | null | undefined): ViewportPreset {
  if (viewport == null) return "auto";
  for (const [name, preset] of [["desktop", desktopViewport], ["mobile", mobileViewport]] as const) {
    if (viewport.width === preset.width && viewport.height === preset.height && viewport.mobile === preset.mobile) return name;
  }
  return "custom";
}

/** Mirrors the native historical defaults; never changes the request identity. */
export function effectiveViewport(config: CrawlConfig): Viewport {
  return config.rendering.viewport ?? (config.agent === "googlebot-smartphone"
    ? { width: 412, height: 915, mobile: true } : { ...desktopViewport });
}
