// Site-wide settings, from the environment (see .env.example).

/** Repository whose GitHub releases are offered; set the variable to `none` to disable. */
const repo = process.env.OCTOPUSSEO_GITHUB_REPO?.trim() || "sabri-kocabas/octopusseo-log-analyser";

export const site = {
  name: "octopusSEO",
  product: "octopusSEO Log Analyser",
  url: (process.env.OCTOPUSSEO_SITE_URL?.trim() || "https://octopusseo.com").replace(/\/$/, ""),
  /** `owner/name` of the GitHub repository with releases; `null` when disabled. */
  repo: repo && /^[\w.-]+\/[\w.-]+$/.test(repo) ? repo : null,
  email: process.env.OCTOPUSSEO_CONTACT_EMAIL?.trim() || null,
} as const;

export const githubUrl = (path = "") => (site.repo ? `https://github.com/${site.repo}${path}` : null);
