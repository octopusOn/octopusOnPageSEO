// Copies the desktop installers and snx-log binaries built on this machine into
// public/downloads/v<version>/ and records them in public/downloads/releases.json. The site
// offers these files while no GitHub repository with releases is configured.
//
//   pnpm --filter @octopusseo/web downloads [--notes <file.md>]
//
// Build first: `pnpm desktop:build` (add `-- --target universal-apple-darwin` on a Mac for
// both Apple Silicon and Intel) and `cargo build --release -p snx-log --target <triple>`;
// CLI binaries are only taken from such explicit-target builds, whose triple names the archive.
// Next.js only serves files that exist when the site is built, so collect before `next build`.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "../../..");
const target = path.join(root, "target");
const downloads = path.resolve(import.meta.dirname, "../public/downloads");
const manifestFile = path.join(downloads, "releases.json");

const { version, productName } = JSON.parse(
  fs.readFileSync(path.join(root, "apps/desktop/src-tauri/tauri.conf.json"), "utf8"),
);
const tag = `v${version}`;
const destination = path.join(downloads, tag);

const INSTALLER = /\.(dmg|msi|exe|appimage|deb|rpm)$/i;
const CLI_TARGET = /^(x86_64|aarch64|i686)-/;

/** Files below `dir`, recursively (`.app` bundles are directories and therefore skipped). */
function files(dir) {
  if (!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) return entry.name.endsWith(".app") ? [] : files(full);
    return entry.isFile() ? [full] : [];
  });
}

/** `target/release` and every `target/<triple>/release`. */
function releaseDirs() {
  const triples = fs.existsSync(target)
    ? fs.readdirSync(target).filter((name) => CLI_TARGET.test(name) || name.startsWith("universal-"))
    : [];
  return [{ dir: path.join(target, "release"), triple: null }].concat(
    triples.map((triple) => ({ dir: path.join(target, triple, "release"), triple })),
  );
}

function sha256(file) {
  return createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

/** GitHub turns spaces in asset names into dots; do the same so URLs stay readable. */
const publicName = (name) => name.replaceAll(" ", ".");

function cliArchive(binary, triple) {
  const name = `snx-log-${tag}-${triple}`;
  const staging = fs.mkdtempSync(path.join(downloads, ".staging-"));
  try {
    const folder = path.join(staging, name);
    fs.mkdirSync(folder);
    fs.copyFileSync(binary, path.join(folder, path.basename(binary)));
    fs.chmodSync(path.join(folder, path.basename(binary)), 0o755);
    for (const doc of ["README.md", "LICENSE-MIT", "LICENSE-APACHE"]) {
      fs.copyFileSync(path.join(root, doc), path.join(folder, doc));
    }
    const archive = path.join(destination, `${name}.tar.gz`);
    execFileSync("tar", ["-czf", archive, "-C", staging, name]);
    return archive;
  } finally {
    fs.rmSync(staging, { recursive: true, force: true });
  }
}

function releaseNotes() {
  const flag = process.argv.indexOf("--notes");
  const file = flag > 0 ? process.argv[flag + 1] : path.join(root, "docs/releases", `${version}.md`);
  return file && fs.existsSync(file) ? fs.readFileSync(file, "utf8").trim() : "";
}

fs.rmSync(destination, { recursive: true, force: true });
fs.mkdirSync(destination, { recursive: true });

const collected = [];
for (const { dir, triple } of releaseDirs()) {
  // Installers of this product and version only; older builds may still sit in the bundle folders.
  for (const file of files(path.join(dir, "bundle"))) {
    const name = path.basename(file);
    const ours = name.startsWith(productName) || name.startsWith(productName.replaceAll(" ", "-"));
    if (!INSTALLER.test(name) || !ours || !(name.includes(`_${version}_`) || name.includes(`-${version}-`))) continue;
    const copy = path.join(destination, publicName(name));
    fs.copyFileSync(file, copy);
    collected.push(copy);
  }
  const binary = ["snx-log", "snx-log.exe"].map((name) => path.join(dir, name)).find((file) => fs.existsSync(file));
  if (binary && triple && CLI_TARGET.test(triple) && isCurrent(binary)) collected.push(cliArchive(binary, triple));
}

/** Whether `binary` reports this version; binaries this machine cannot run are trusted. */
function isCurrent(binary) {
  let output;
  try {
    output = execFileSync(binary, ["--version"], { encoding: "utf8" });
  } catch {
    console.warn(`warning: cannot run ${path.relative(root, binary)} to check its version`);
    return true;
  }
  if (output.trim().endsWith(` ${version}`)) return true;
  console.warn(`skipping ${path.relative(root, binary)}: reports "${output.trim()}", not ${version}`);
  return false;
}

if (collected.length === 0) {
  fs.rmSync(destination, { recursive: true, force: true });
  console.error(`No ${productName} ${version} installers or snx-log binaries found under ${target}.`);
  process.exit(1);
}

const entry = {
  version,
  name: `${productName} ${version}`,
  date: new Date().toISOString(),
  notes: releaseNotes(),
  assets: collected
    .map((file) => ({ name: path.basename(file), size: fs.statSync(file).size, sha256: sha256(file) }))
    .sort((a, b) => a.name.localeCompare(b.name)),
};
const manifest = fs.existsSync(manifestFile) ? JSON.parse(fs.readFileSync(manifestFile, "utf8")) : { releases: [] };
manifest.releases = [entry, ...manifest.releases.filter((release) => release.version !== version)];
fs.writeFileSync(manifestFile, `${JSON.stringify(manifest, null, 2)}\n`);

for (const asset of entry.assets) {
  console.log(`${asset.name}  ${(asset.size / 1024 / 1024).toFixed(1)} MB  ${asset.sha256}`);
}
console.log(`\n${entry.assets.length} files → ${path.relative(root, destination)}`);
