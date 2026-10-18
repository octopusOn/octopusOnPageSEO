import { ResetRequestForm } from "@/components/AuthForms";
import { getDictionary } from "@/i18n";

export default async function Page() {
  const { t } = await getDictionary();
  return <ResetRequestForm t={t.auth} />;
}
