// The seller in the legal texts, from the environment. The CMS writes the texts with
// {{company.title}}-style fields; missing details show as bracketed placeholders.

export interface Company {
  title?: string;
  address?: string;
  tax_office?: string;
  tax_number?: string;
  mersis?: string;
  email: string;
  phone?: string;
  kep?: string;
  /** The portal's host: "my.octopuson.com". */
  site: string;
}

export function company(env: NodeJS.ProcessEnv = process.env): Company {
  const site = (env.BETTER_AUTH_URL || "https://my.octopuson.com").replace(/\/+$/, "");
  return {
    title: env.COMPANY_TITLE || undefined,
    address: env.COMPANY_ADDRESS || undefined,
    tax_office: env.COMPANY_TAX_OFFICE || undefined,
    tax_number: env.COMPANY_TAX_NUMBER || undefined,
    mersis: env.COMPANY_MERSIS || undefined,
    email: env.COMPANY_EMAIL || "destek@octopuson.com",
    phone: env.COMPANY_PHONE || undefined,
    kep: env.COMPANY_KEP || undefined,
    site: site.replace(/^https?:\/\//, ""),
  };
}

/** The texts are drafts until a lawyer has reviewed them (LEGAL_APPROVED=1). */
export function legalApproved(env: NodeJS.ProcessEnv = process.env): boolean {
  return env.LEGAL_APPROVED === "1";
}

/** The language whose text is binding; the others are translations (LEGAL_LANGUAGE, Turkish unless set). */
export function legalLanguage(env: NodeJS.ProcessEnv = process.env): string {
  return env.LEGAL_LANGUAGE || "tr";
}

/** Fills `{{company.title}}` and the other fields; `missing` names each absent detail. */
export function fillCompany(markdown: string, seller: Company, missing: Record<string, string>): string {
  return markdown.replace(/\{\{\s*company\.([a-z_]+)\s*\}\}/g, (field, name: string) => {
    const value = (seller as unknown as Record<string, string | undefined>)[name];
    if (value) return value;
    return missing[name] ? `[${missing[name]}]` : field;
  });
}
