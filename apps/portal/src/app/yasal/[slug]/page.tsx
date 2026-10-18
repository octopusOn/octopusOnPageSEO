import Link from "next/link";
import { notFound } from "next/navigation";

import { BrandMark } from "@/components/BrandMark";
import { company, legalApproved } from "@/legal/company";
import { legalDocuments } from "@/legal/documents";

export async function generateMetadata({ params }: { params: Promise<{ slug: string }> }) {
  const { slug } = await params;
  return { title: legalDocuments(company()).find((doc) => doc.slug === slug)?.title ?? "octopusSEO" };
}

export default async function Legal({ params }: { params: Promise<{ slug: string }> }) {
  const { slug } = await params;
  const documents = legalDocuments(company());
  const doc = documents.find((item) => item.slug === slug);
  if (!doc) notFound();
  return (
    <div className="oc-ground legal-ground">
      <article className="legal">
        <Link href="/" className="oc-brand legal-brand">
          <BrandMark />
          <span className="oc-brand-name">
            octopus<span>SEO</span>
          </span>
        </Link>
        {!legalApproved() ? (
          <div className="oc-banner warning">Taslak metin: hukukçu incelemesinden sonra yürürlüğe girecektir.</div>
        ) : null}
        <h1>{doc.title}</h1>
        {doc.sections.map((section) => (
          <section key={section.heading}>
            <h2>{section.heading}</h2>
            {section.body.map((paragraph) => (
              <p key={paragraph.slice(0, 40)}>{paragraph}</p>
            ))}
          </section>
        ))}
        <nav className="legal-nav small">
          {documents.map((item) => (
            <Link key={item.slug} href={`/yasal/${item.slug}`} aria-current={item.slug === slug ? "page" : undefined}>
              {item.title}
            </Link>
          ))}
        </nav>
      </article>
    </div>
  );
}
