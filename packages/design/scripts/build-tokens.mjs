// Generates tokens.css from tokens.json, the single source of every design value.
//
//   node scripts/build-tokens.mjs           write tokens.css
//   node scripts/build-tokens.mjs --check   fail when tokens.css is out of date (CI)
//
// The output follows the layout the octopusSEO Design System page compiles, so the same
// component CSS works in the apps and on that page: the first theme on `:root`, every further
// theme under `[data-theme="<id>"]`, plain `--<name>` properties and a `.<style>` class per
// type style. On top of that, a theme-less root follows the operating system's color scheme.

import fs from "node:fs";
import path from "node:path";

const dir = path.resolve(import.meta.dirname, "..");
const tokens = JSON.parse(fs.readFileSync(path.join(dir, "tokens.json"), "utf8"));
const output = path.join(dir, "tokens.css");

// Unicode ranges of the Google Fonts subsets the font files were cut to.
const RANGES = {
  latin:
    "U+0000-00FF, U+0131, U+0152-0153, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308, U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215, U+FEFF, U+FFFD",
  "latin-ext":
    "U+0100-02BA, U+02BD-02C5, U+02C7-02CC, U+02CE-02D7, U+02DD-02FF, U+0304, U+0308, U+0329, U+1D00-1DBF, U+1E00-1E9F, U+1EF2-1EFF, U+2020, U+20A0-20AB, U+20AD-20C0, U+2113, U+2C60-2C7F, U+A720-A7FF",
};

const [first, ...others] = tokens.color.themes.map((theme) => theme.id);
const themed = [...tokens.color.tokens, ...(tokens.shadow?.tokens ?? [])];

/** Value of a color or shadow token in `theme`; a missing theme inherits the first. */
function valueIn(token, theme) {
  if (typeof token.value === "string") return token.value;
  const value = token.value[theme] ?? (theme === first ? undefined : token.value[first]);
  return typeof value === "string" ? value.replace(/^\{(.+)\}$/, "var(--$1)") : value;
}

function themeBlock(theme, indent) {
  const lines = themed.map((token) => `${indent}--${token.name}: ${valueIn(token, theme)};`);
  lines.push(`${indent}color-scheme: ${theme === "light" ? "light" : "dark"};`);
  return lines.join("\n");
}

/** A theme named after a color scheme also applies when the system asks for it. */
function systemBlock(theme) {
  if (theme !== "light" && theme !== "dark") return [];
  return [`@media (prefers-color-scheme: ${theme}) {\n  :root:not([data-theme]) {\n${themeBlock(theme, "    ")}\n  }\n}`, ""];
}

function fontFaces() {
  return tokens.type.fonts.flatMap((font) => {
    const faces = [];
    for (const [subset, range] of Object.entries(RANGES)) {
      const file = font.file.replace("-latin-", `-${subset}-`);
      if (!fs.existsSync(path.join(dir, file))) continue;
      faces.push(
        [
          "@font-face {",
          `  font-family: "${font.family}";`,
          `  src: url("./${file}") format("woff2");`,
          `  font-weight: ${font.weight};`,
          `  font-style: ${font.style ?? "normal"};`,
          "  font-display: swap;",
          `  unicode-range: ${range};`,
          "}",
        ].join("\n"),
      );
    }
    return faces;
  });
}

function typeClasses() {
  return tokens.type.groups.flatMap((group) =>
    group.styles.map((style) => {
      const family = style.family ?? group.family;
      const rules = [`  font-family: var(--font-${family});`, `  font-size: ${style.fontSize};`];
      if (style.lineHeight) rules.push(`  line-height: ${style.lineHeight};`);
      if (style.fontWeight) rules.push(`  font-weight: ${style.fontWeight};`);
      if (style.letterSpacing) rules.push(`  letter-spacing: ${style.letterSpacing};`);
      return `.${style.name} {\n${rules.join("\n")}\n}`;
    }),
  );
}

const plain = Object.entries(tokens)
  .filter(([family, value]) => !["color", "type", "shadow"].includes(family) && Array.isArray(value?.tokens))
  .flatMap(([, value]) => value.tokens.map((token) => `  --${token.name}: ${token.value};`));
const families = Object.entries(tokens.type.families).map(([key, stack]) => `  --font-${key}: ${stack};`);

const css = [
  `/* ${tokens.name} — generated from tokens.json */`,
  "/* Do not edit by hand: change tokens.json and run `pnpm --filter @octopusseo/design build`. */",
  "",
  ...fontFaces().flatMap((face) => [face, ""]),
  `:root,\n[data-theme="${first}"] {\n${themeBlock(first, "  ")}\n}`,
  "",
  ...others.flatMap((theme) => [`[data-theme="${theme}"] {\n${themeBlock(theme, "  ")}\n}`, ""]),
  ...others.flatMap(systemBlock),
  `:root {\n${[...plain, ...families].join("\n")}\n}`,
  "",
  ...typeClasses().flatMap((rule) => [rule, ""]),
].join("\n");

if (process.argv.includes("--check")) {
  const current = fs.existsSync(output) ? fs.readFileSync(output, "utf8") : "";
  if (current !== css) {
    console.error("tokens.css is out of date: run `pnpm --filter @octopusseo/design build`.");
    process.exit(1);
  }
  console.log("tokens.css is up to date.");
} else {
  fs.writeFileSync(output, css);
  console.log(`Wrote ${path.relative(process.cwd(), output)}.`);
}
