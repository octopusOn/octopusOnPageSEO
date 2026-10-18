import Link from "next/link";
import { Download, Star } from "lucide-react";

import type { Dictionary } from "@/i18n";
import type { Language, Navigation } from "@/lib/cms";
import { formatCompact } from "@/lib/format";
import { repoStars } from "@/lib/releases";
import { githubUrl, site } from "@/lib/site";
import { GithubIcon } from "./icons";
import { LanguageSwitcher } from "./LanguageSwitcher";
import { Logo } from "./Logo";
import { MobileMenu } from "./MobileMenu";
import { ProgramsMenu } from "./ProgramsMenu";

/**
 * Logo, the program menu and the CMS's header links, then the GitHub star button, the language
 * switcher and the download button.
 */
export async function Header({ lang, dict, navigation, languages }: { lang: string; dict: Dictionary; navigation: Navigation; languages: Language[] }) {
  const github = githubUrl();
  const stars = github ? await repoStars() : null;
  return (
    <header className="header">
      <nav className="container nav" aria-label="Main">
        <Link href={`/${lang}`} className="nav-home" aria-label={site.name}>
          <Logo />
        </Link>
        <MobileMenu label={dict.nav.menu}>
          <ProgramsMenu label={dict.nav.programs} programs={navigation.programs} />
          {navigation.header.map((link) => (
            <Link key={link.url} href={link.url}>
              {link.label}
            </Link>
          ))}
        </MobileMenu>
        <div className="nav-end">
          {github && (
            <a href={github} rel="noopener" className="github-star" aria-label={dict.nav.starLabel} title={dict.nav.starLabel}>
              <span className="github-star-label">
                <GithubIcon size={16} />
                <Star size={14} aria-hidden="true" />
                <span>{dict.nav.star}</span>
              </span>
              {stars !== null && <span className="github-star-count">{formatCompact(stars, lang)}</span>}
            </a>
          )}
          <LanguageSwitcher current={lang} label={dict.nav.language} languages={languages} />
          {navigation.download && (
            <Link href={navigation.download} className="button button-small button-primary nav-cta">
              <Download size={16} aria-hidden="true" />
              <span className="nav-cta-label">{dict.nav.download}</span>
            </Link>
          )}
        </div>
      </nav>
    </header>
  );
}
