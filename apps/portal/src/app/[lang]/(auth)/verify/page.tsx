import { ResendVerification } from "@/components/AuthForms";
import { getDictionary } from "@/i18n";

export default async function Page({ searchParams }: { searchParams: Promise<{ e?: string }> }) {
  const [{ t }, { e }] = await Promise.all([getDictionary(), searchParams]);
  return <ResendVerification t={t.auth} email={e ?? null} />;
}
