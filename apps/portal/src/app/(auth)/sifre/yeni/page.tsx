import { ResetForm } from "@/components/AuthForms";
import { getDictionary } from "@/i18n";

export default async function Page({ searchParams }: { searchParams: Promise<{ token?: string; error?: string }> }) {
  const [{ t }, { token, error }] = await Promise.all([getDictionary(), searchParams]);
  return <ResetForm t={t.auth} token={error ? null : (token ?? null)} />;
}
