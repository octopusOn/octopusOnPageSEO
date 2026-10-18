import Link from "next/link";

import type { Dictionary } from "@/i18n";
import type { Navigation, NavLink } from "@/lib/cms";
import { format } from "@/lib/format";
import { githubUrl, site } from "@/lib/site";
import { Logo } from "./Logo";

function Column({ title, links, extra }: { title: string; links: NavLink[]; extra?: { href: string; label: string }[] }) {
  const external = extra ?? [];
  if (links.length === 0 && external.length === 0) return null;
  return (
    <div>
      <h2>{title}</h2>
      <ul>
        {links.map((link) => (
          <li key={link.url}>
            <Link href={link.url}>{link.label}</Link>
          </li>
        ))}
        {external.map((link) => (
          <li key={link.href}>
            <a href={link.href} rel={link.href.startsWith("mailto:") ? undefined : "noopener"}>
              {link.label}
            </a>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** The CMS's footer columns, plus GitHub, contact and issue links from the site settings. */
export function Footer({ dict, navigation }: { lang: string; dict: Dictionary; navigation: Navigation }) {
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
        <Column title={f.product} links={navigation.footer.product} />
        <Column title={f.resources} links={navigation.footer.resources} extra={github ? [{ href: github, label: dict.nav.github }] : []} />
        <Column
          title={f.company}
          links={navigation.footer.company}
          extra={[
            ...(site.email ? [{ href: `mailto:${site.email}`, label: f.contact }] : []),
            ...(issues ? [{ href: issues, label: f.issues }] : []),
          ]}
        />
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
