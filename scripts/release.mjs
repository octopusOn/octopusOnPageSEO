#!/usr/bin/env node
// Prepares the release of one OctopusOn SEO program (docs/release.md, ADR 0074):
//
//   pnpm release <program> [patch|minor|major|<x.y.z>] [--dry-run] [--tag] [--since=<ref>]
//
// <program> is a key or id from apps/web/src/lib/products.json (`log`, `spider`, `serp`,
// `keywords`, `index`). Every program has its own Semantic Version and its own tags
// (`spider-v0.2.0`). The script:
//   1. collects the commits since the program's last tag that belong to it: Conventional Commits
//      scoped to the program (`feat(spider): …`) or touching its folders (`paths`);
//   2. picks the next version from them (breaking → major, feat → minor, else patch; before 1.0
//      a breaking change is a minor) unless one is given;
//   3. writes the English release notes, docs/releases/<program>-<version>.md (the GitHub
//      release body and the website's changelog), and prepends them to
//      docs/changelog/<program>.md (Keep a Changelog);
//   4. sets the version in the app's package.json and src-tauri/tauri.conf.json, the CLI's
//      Cargo.toml and Cargo.lock;
//   5. with --tag, commits those files and creates the annotated tag. Pushing stays manual.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const read = (file) => readFileSync(path.join(root, file), "utf8");
const write = (file, text) => writeFileSync(path.join(root, file), text);
const git = (...args) => execFileSync("git", args, { cwd: root, encoding: "utf8" }).trim();

// ---------------------------------------------------------------- arguments
const args = process.argv.slice(2);
const flags = new Set(args.filter((a) => a.startsWith("--")));
const [programArg, bumpArg] = args.filter((a) => !a.startsWith("--"));
const products = JSON.parse(read("apps/web/src/lib/products.json"));
const product = products.find((p) => p.key === programArg || p.id === programArg);
if (!product) {
  console.error(`Usage: pnpm release <${products.map((p) => p.key).join("|")}> [patch|minor|major|x.y.z] [--dry-run] [--tag]`);
  process.exit(1);
}
const dryRun = flags.has("--dry-run");

// ---------------------------------------------------------------- versions
const SEMVER = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$/;
const parse = (v) => {
  const m = SEMVER.exec(v);
  return m ? { major: +m[1], minor: +m[2], patch: +m[3], pre: m[4] ?? null } : null;
};
const compare = (a, b) => a.major - b.major || a.minor - b.minor || a.patch - b.patch || (a.pre ? (b.pre ? a.pre.localeCompare(b.pre) : -1) : b.pre ? 1 : 0);

const tauriConf = `${product.app}/src-tauri/tauri.conf.json`;
const current = JSON.parse(read(tauriConf)).version;

const prefixes = [product.tagPrefix, ...(product.legacyTagPrefixes ?? [])];
const tags = git("tag", "--list")
  .split("\n")
  .flatMap((tag) => {
    const prefix = prefixes.find((p) => tag.startsWith(p) && parse(tag.slice(p.length)));
    return prefix ? [{ tag, version: parse(tag.slice(prefix.length)) }] : [];
  })
  .sort((a, b) => compare(b.version, a.version));
// --since=<ref> previews the notes from another starting point.
const since = args.find((a) => a.startsWith("--since="))?.slice(8) ?? null;
const lastTag = since ?? tags[0]?.tag ?? null;

// ---------------------------------------------------------------- commits
const CONVENTIONAL = /^(\w+)(?:\(([^)]+)\))?(!)?:\s*(.+)$/;
const scopes = new Set([product.key, product.id, product.package, product.cli]);
const SEP = "\u001e";
const range = lastTag ? `${lastTag}..HEAD` : "HEAD";
const log = git("log", range, `--format=${SEP}%h%x1f%s%x1f%b%x1f`, "--name-only");
const commits = log
  .split(SEP)
  .filter(Boolean)
  .map((entry) => {
    const [hash, subject, body, files = ""] = entry.split("\u001f");
    const m = CONVENTIONAL.exec(subject.trim());
    return {
      hash,
      subject: subject.trim(),
      body: body.trim(),
      files: files.split("\n").map((f) => f.trim()).filter(Boolean),
      type: m?.[1]?.toLowerCase() ?? null,
      scope: m?.[2] ?? null,
      breaking: Boolean(m?.[3]) || /^BREAKING[ -]CHANGE:/m.test(body),
      text: m?.[4] ?? subject.trim(),
    };
  })
  .filter((c) => {
    const scoped = c.scope?.split(/[,\s]+/).some((s) => scopes.has(s));
    const touches = c.files.some((f) => product.paths.some((p) => f === p || f.startsWith(`${p}/`)));
    return scoped || touches;
  });

const SECTIONS = [
  ["Breaking changes", (c) => c.breaking],
  ["Added", (c) => c.type === "feat"],
  ["Fixed", (c) => c.type === "fix"],
  ["Performance", (c) => c.type === "perf"],
  ["Changed", (c) => c.type === "refactor" || c.type === "revert" || c.type === null],
  ["Security", (c) => c.type === "security" || (c.type === "fix" && /security|cve/i.test(c.text))],
];
// chore, ci, test, docs, style and build commits stay out of the notes unless they break something.
const sections = SECTIONS.map(([title, test]) => {
  const items = commits.filter((c) => test(c) && (title === "Breaking changes" || !c.breaking));
  return { title, items: title === "Fixed" ? items.filter((c) => !/security|cve/i.test(c.text)) : items };
}).filter((s) => s.items.length > 0);

// ---------------------------------------------------------------- next version
const base = parse(current) ?? { major: 0, minor: 1, patch: 0, pre: null };
function bumped(kind) {
  if (kind === "major") return base.major === 0 ? `0.${base.minor + 1}.0` : `${base.major + 1}.0.0`;
  if (kind === "minor") return `${base.major}.${base.minor + 1}.0`;
  return `${base.major}.${base.minor}.${base.patch + 1}`;
}
const auto = commits.some((c) => c.breaking) ? "major" : commits.some((c) => c.type === "feat") ? "minor" : "patch";
let version;
if (bumpArg && parse(bumpArg)) version = bumpArg;
else if (!bumpArg && !lastTag) version = current; // the first release ships the version already set
else version = bumped(bumpArg ?? auto);
if (tags[0] && parse(version) && compare(parse(version), tags[0].version) <= 0) {
  console.error(`${version} is not newer than ${lastTag}.`);
  process.exit(1);
}
const tag = `${product.tagPrefix}${version}`;

// ---------------------------------------------------------------- notes
const today = new Date().toISOString().slice(0, 10);
const capital = (t) => t.charAt(0).toUpperCase() + t.slice(1);
const bullet = (c) => `- ${capital(c.text.replace(/\.$/, ""))} (${c.hash})`;
const body = sections.map((s) => `## ${s.title}\n\n${s.items.map(bullet).join("\n")}`).join("\n\n");
const notesFile = `docs/releases/${product.key}-${version}.md`;
const install = [
  "## Install",
  "",
  `Download the installer for your system from the assets below: \`.dmg\` (macOS, universal), \`.msi\` or \`.exe\` (Windows), \`.AppImage\`, \`.deb\` or \`.rpm\` (Linux).`,
  `The command-line tool \`${product.cli}\` is in the \`${product.cli}-${tag}-<target>\` archives.`,
  "",
  `**macOS:** until the app is notarized, macOS says Apple could not verify it is free of malware. Drag it to Applications and run \`xattr -dr com.apple.quarantine "/Applications/${product.name}.app"\` once in Terminal, or choose **Open Anyway** in System Settings → Privacy & Security.`,
].join("\n");
const notes = [
  `# ${product.name} ${version}`,
  "",
  "<!-- Highlights: replace this comment with two or three sentences on what matters most. -->",
  "",
  body || "_No user-facing changes were recorded since the previous release._",
  "",
  install,
  "",
].join("\n");

const changelogFile = `docs/changelog/${product.key}.md`;
const changelogHead = `# ${product.name} changelog\n\nAll notable changes, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the program uses [Semantic Versioning](https://semver.org/).\n`;
const entry = `## [${version}] - ${today}\n\n${(body || "_No user-facing changes._").replace(/^## /gm, "### ")}\n`;

console.log(`${product.name}: ${current} → ${version} (${tag}); ${commits.length} commits since ${lastTag ?? "the beginning"}`);
for (const s of sections) console.log(`  ${s.title}: ${s.items.length}`);

if (dryRun) {
  console.log(`\n${notes}`);
  process.exit(0);
}

if (existsSync(path.join(root, notesFile))) {
  console.log(`${notesFile} exists; kept as written.`);
} else {
  write(notesFile, notes);
  console.log(`Wrote ${notesFile}`);
}
mkdirSync(path.join(root, "docs/changelog"), { recursive: true });
const existing = existsSync(path.join(root, changelogFile)) ? read(changelogFile) : changelogHead;
if (!existing.includes(`## [${version}]`)) {
  const [head, ...rest] = existing.split(/\n(?=## \[)/);
  write(changelogFile, [head.trimEnd() + "\n", entry, ...rest].join("\n"));
  console.log(`Updated ${changelogFile}`);
}

// ---------------------------------------------------------------- version numbers
// Every file the release touches; `--tag` commits exactly these.
const files = [notesFile, changelogFile];
const changed = [];
function setJsonVersion(file) {
  const text = read(file);
  const next = text.replace(/("version":\s*")[^"]+(")/, `$1${version}$2`);
  files.push(file);
  if (next !== text) {
    write(file, next);
    changed.push(file);
  }
}
setJsonVersion(`${product.app}/package.json`);
setJsonVersion(tauriConf);

// The CLI gets its own version instead of the workspace's; Cargo.lock follows, so `--locked`
// builds keep working.
const cliManifest = `apps/${product.package}/Cargo.toml`;
if (existsSync(path.join(root, cliManifest))) {
  const text = read(cliManifest);
  const next = text.replace(/^version(?:\.workspace)?\s*=.*$/m, `version = "${version}"`);
  files.push(cliManifest, "Cargo.lock");
  if (next !== text) {
    write(cliManifest, next);
    changed.push(cliManifest);
  }
  const lock = read("Cargo.lock");
  const nextLock = lock.replace(new RegExp(`(\\[\\[package\\]\\]\\nname = "${product.package}"\\nversion = ")[^"]+(")`), `$1${version}$2`);
  if (nextLock !== lock) {
    write("Cargo.lock", nextLock);
    changed.push("Cargo.lock");
  }
}
console.log(`Version ${version} set in: ${changed.join(", ") || "(already set)"}`);

// ---------------------------------------------------------------- commit and tag
if (flags.has("--tag")) {
  git("add", "--", ...files);
  git("commit", "-m", `release(${product.key}): ${product.name} ${version}`, "--", ...files);
  git("tag", "-a", tag, "-m", `${product.name} ${version}`);
  console.log(`\nCommitted and tagged ${tag}. Publish with:\n  git push origin HEAD ${tag}`);
} else {
  console.log(`\nNext:\n  1. Edit ${notesFile}: write the highlights, polish the bullets.\n  2. cargo xtask ci\n  3. pnpm release ${product.key} ${version} --tag   (commits these files and tags ${tag})\n  4. git push origin HEAD ${tag}`);
}
