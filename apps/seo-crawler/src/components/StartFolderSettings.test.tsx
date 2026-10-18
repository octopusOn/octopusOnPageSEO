import { renderToStaticMarkup } from "react-dom/server";
import { expect, it, vi } from "vitest";
import { defaultConfig } from "../api/types";
import { validConfig } from "../lib/config";
import { StartFolderSettings } from "./StartFolderSettings";

it("keeps old plans unrestricted and renders an explicit folder choice without changing its source", () => {
  const config = structuredClone(defaultConfig);
  const before = structuredClone(config);
  const onChange = vi.fn();
  const html = renderToStaticMarkup(<StartFolderSettings config={config} onChange={onChange} />);
  expect(html).toContain("Başlangıç klasörlerinin içinde kal");
  expect(html).toContain("/tr/");
  expect(html).not.toContain("checked");
  expect(onChange).not.toHaveBeenCalled();
  expect(config).toEqual(before);
  const enabled = { ...config, urls: { ...config.urls, start_folder_only: true } };
  expect(renderToStaticMarkup(<StartFolderSettings config={enabled} onChange={onChange} disabled />)).toContain('disabled="" checked=""');
  expect(validConfig(enabled)).toBe(true);
});

it("accepts missing legacy and explicit boolean folder flags, rejecting malformed persisted flags", () => {
  for (const value of [undefined, false, true]) expect(validConfig({ ...defaultConfig, urls: { ...defaultConfig.urls, start_folder_only: value } })).toBe(true);
  for (const value of [null, "false", 0]) expect(validConfig({ ...defaultConfig, urls: { ...defaultConfig.urls, start_folder_only: value as unknown as boolean } })).toBe(false);
});
