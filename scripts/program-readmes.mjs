#!/usr/bin/env node
// Writes distribution/<program id>/README.md and screenshot.jpg, the front page of each program's
// own GitHub project (products.json `repo`), from the English content of the CMS seed. The
// programs workflow copies distribution/<id>/ into that repository. Edit the README here or
// rerun `node scripts/program-readmes.mjs` after changing the program's page.

import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const products = JSON.parse(readFileSync(path.join(root, "apps/web/src/lib/products.json"), "utf8"));
const seed = JSON.parse(readFileSync(path.join(root, "apps/cms/database/seeders/content.json"), "utf8"));
const SOURCE = "octopusOn/octopusOnPageSEO";

for (const product of products) {
  const page = seed.pages.find((p) => p.product === product.id);
  const en = page?.translations.en;
  if (!en) continue;
  const block = (type) => en.blocks.find((b) => b.type === type)?.data;
  const intro = block("hero")?.subtitle || block("page_header")?.subtitle || en.excerpt || "";
  const sections = block("sections")?.items ?? [];
  const code = block("code")?.code ?? `${product.cli} --help`;
  const url = `https://octopuson.com/en/${en.slug}`;
  const readme = [
    `# ${product.name}`,
    "",
    `${intro}`,
    "",
    `**100% free and open source** (no paid version, no trial, no limits) · for SEO and GEO (visibility in AI answers) · Windows, macOS and Linux · updated continuously as search engines and AI assistants change.`,
    "",
    `![${product.name}](screenshot.jpg)`,
    "",
    "## Download",
    "",
    `Get the installer for your system from the [latest release](../../releases/latest): \`.dmg\` for macOS (Apple Silicon and Intel), \`.msi\` or \`.exe\` for Windows, \`.AppImage\`, \`.deb\` or \`.rpm\` for Linux. More on [octopuson.com](${url}).`,
    "",
    ...(sections.length
      ? ["## What it does", "", ...sections.flatMap((s) => [`### ${s.title}`, "", s.text, "", ...(s.bullets ?? []).map((b) => `- ${b}`), ""])]
      : []),
    "## Command line",
    "",
    `Every feature also works from the terminal with \`${product.cli}\`, included in the release archives:`,
    "",
    "```bash",
    code,
    "```",
    "",
    "## Your own APIs or OctopusOn credits",
    "",
    "Everything that runs on your computer is free. Search APIs are optional: connect your own keys (DataForSEO, Serper, SerpApi, Yandex), or use all of them with prepaid credits through one connection at [my.octopuson.com](https://my.octopuson.com).",
    "",
    "## Source code and issues",
    "",
    `This project publishes the releases of the ${product.short}. Its source code lives in [${SOURCE}](https://github.com/${SOURCE}), together with the engine every OctopusOn SEO program shares. Bug reports and ideas are welcome in this project's issues.`,
    "",
    "## Licence",
    "",
    "MIT or Apache-2.0, at your option.",
    "",
  ].join("\n");
  const dir = path.join(root, "distribution", product.id);
  mkdirSync(dir, { recursive: true });
  writeFileSync(path.join(dir, "README.md"), readme);
  const shot = path.join(root, "apps/cms/database/seeders/media", `${product.id}.jpg`);
  if (existsSync(shot)) copyFileSync(shot, path.join(dir, "screenshot.jpg"));
  console.log(`distribution/${product.id}/README.md`);
}
