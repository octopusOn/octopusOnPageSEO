import Link from "next/link";

import type { Locale } from "@/i18n/config";
import type { Dictionary } from "@/i18n";
import { format } from "@/lib/format";
import { githubUrl, site } from "@/lib/site";
import { Logo } from "./Logo";

export function Footer({ lang, dict }: { lang: Locale; dict: Dictionary }) {
  const f = dict.footer;
  const github = githubUrl();
  const issues = githubUrl("/issues");
  return (
    <footer className="footer">
      <div className="container footer-grid">
        <div className="footer-brand">
          <Logo />
          <p>{f.tagline}</p>
        </div>
        <div>
          <h2>{f.product}</h2>
          <ul>
            <li>
              <Link href={`/${lang}/features`}>{f.logAnalyser}</Link>
            </li>
            <li>
              <span className="muted">
                {f.spider} <span className="pill">{f.comingSoon}</span>
              </span>
            </li>
            <li>
              <Link href={`/${lang}/download`}>{dict.nav.download}</Link>
            </li>
          </ul>
        </div>
        <div>
          <h2>{f.resources}</h2>
          <ul>
            <li>
              <Link href={`/${lang}/docs`}>{dict.nav.docs}</Link>
            </li>
            <li>
              <Link href={`/${lang}/changelog`}>{dict.nav.changelog}</Link>
            </li>
            {github && (
              <li>
                <a href={github} rel="noopener">
                  {dict.nav.github}
                </a>
              </li>
            )}
          </ul>
        </div>
        {(site.email || issues) && (
          <div>
            <h2>{f.company}</h2>
            <ul>
              {site.email && (
                <li>
                  <a href={`mailto:${site.email}`}>{f.contact}</a>
                </li>
              )}
              {issues && (
                <li>
                  <a href={issues} rel="noopener">
                    {f.issues}
                  </a>
                </li>
              )}
            </ul>
          </div>
        )}
      </div>
      <div className="container footer-bottom">
        <span>{format(f.rights, { year: new Date().getFullYear() })}</span>
        <a href="https://db-ip.com" rel="noopener">
          {f.geo}
        </a>
      </div>
    </footer>
  );
}
