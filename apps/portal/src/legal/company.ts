// The seller's details for the legal texts, from the environment; brackets show what is missing.

export interface Company {
  title: string;
  address: string;
  taxOffice: string;
  taxNumber: string;
  mersis: string;
  email: string;
  phone: string;
  kep: string;
  site: string;
}

export function company(env: NodeJS.ProcessEnv = process.env): Company {
  return {
    title: env.COMPANY_TITLE ?? "[Şirket unvanı]",
    address: env.COMPANY_ADDRESS ?? "[Şirket adresi]",
    taxOffice: env.COMPANY_TAX_OFFICE ?? "[Vergi dairesi]",
    taxNumber: env.COMPANY_TAX_NUMBER ?? "[Vergi kimlik no]",
    mersis: env.COMPANY_MERSIS ?? "[MERSİS no]",
    email: env.COMPANY_EMAIL ?? "destek@octopuson.com",
    phone: env.COMPANY_PHONE ?? "[Telefon]",
    kep: env.COMPANY_KEP ?? "[KEP adresi]",
    site: (env.BETTER_AUTH_URL ?? "https://my.octopuson.com").replace(/\/+$/, ""),
  };
}

/** The texts are drafts until a lawyer has reviewed them (LEGAL_APPROVED=1). */
export function legalApproved(env: NodeJS.ProcessEnv = process.env): boolean {
  return env.LEGAL_APPROVED === "1";
}
