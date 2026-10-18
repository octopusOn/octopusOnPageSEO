import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Coins, LogIn, LogOut, MonitorSmartphone } from "lucide-react";

import { api } from "../api/commands";
import type { CommandError } from "../api/types";
import { useI18n } from "../i18n";
import { formatNumber } from "../lib/format";
import { keys } from "../state";
import { Button, Card, ErrorBanner } from "./ui";

/** The OctopusOn SEO account every program on this computer shares: one click connects them all. */
export function AccountCard() {
  const { t, locale } = useI18n();
  const queryClient = useQueryClient();
  // Another OctopusOn SEO program may connect or disconnect the account meanwhile.
  const status = useQuery({ queryKey: keys.account, queryFn: api.accountStatus, refetchInterval: 30_000 });
  const refresh = () => void queryClient.invalidateQueries({ queryKey: keys.account });
  const connect = useMutation<boolean, CommandError>({ mutationFn: api.accountConnect, onSettled: refresh });
  const logout = useMutation<void, CommandError>({ mutationFn: api.accountLogout, onSettled: refresh });
  const open = useMutation<void, CommandError, "credits" | "devices">({ mutationFn: api.accountOpen });
  const cloud = status.data;
  const account = cloud?.account;
  return (
    <Card title={t("account.title")}>
      <div className="stack">
        <p className="muted small">{t("account.intro")}</p>
        {cloud?.error && <div className="oc-banner warning">{cloud.error}</div>}
        {account?.low_balance && <div className="oc-banner warning">{t("account.low")}</div>}
        <div className="row between">
          <div>
            <strong>{cloud?.signed_in ? (account?.email ?? "") : t("account.notConnected")}</strong>
            {account && (
              <div className="muted">
                {t("account.balance", {
                  credits: formatNumber(account.available_micros / account.credit_unit_micros, locale),
                })}
              </div>
            )}
          </div>
          <div className="row">
            {cloud?.signed_in ? (
              <>
                <Button variant="primary" icon={<Coins size={15} />} onClick={() => open.mutate("credits")}>
                  {t("account.buy")}
                </Button>
                <Button icon={<MonitorSmartphone size={15} />} onClick={() => open.mutate("devices")}>
                  {t("account.devices")}
                </Button>
                <Button icon={<LogOut size={15} />} busy={logout.isPending} onClick={() => logout.mutate()}>
                  {t("account.signOut")}
                </Button>
              </>
            ) : connect.isPending ? (
              <>
                <span className="muted small">{t("account.waiting")}</span>
                <Button onClick={() => void api.accountCancel()}>{t("account.cancel")}</Button>
              </>
            ) : (
              <Button variant="primary" icon={<LogIn size={15} />} onClick={() => connect.mutate()}>
                {t("account.connect")}
              </Button>
            )}
          </div>
        </div>
        <ErrorBanner error={connect.error ?? logout.error ?? open.error} />
      </div>
    </Card>
  );
}
