import { TwoFactorForm } from "@/components/AuthForms";
import { getDictionary } from "@/i18n";

export default async function Page({ searchParams }: { searchParams: Promise<{ next?: string }> }) {
  const [{ t }, { next }] = await Promise.all([getDictionary(), searchParams]);
  return <TwoFactorForm t={t.auth} next={next ?? null} />;
}
