import { ResendVerification } from "@/components/AuthForms";
import { getDictionary } from "@/i18n";

export default async function Page({ searchParams }: { searchParams: Promise<{ email?: string }> }) {
  const [{ t }, { email }] = await Promise.all([getDictionary(), searchParams]);
  return <ResendVerification t={t.auth} email={email ?? null} />;
}
