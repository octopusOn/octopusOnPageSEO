// The assistant that writes: Claude, ChatGPT or Gemini with the user's own key, with what it
// would cost. Writing on credits is not offered yet; without a key the picker leads to Sources.

import { useState } from "react";
import { PenLine, PlugZap } from "lucide-react";
import { Button } from "@octopusseo/ui";
import type { Writer, WriterEngine } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { formatUsd } from "../../lib/format";

const NAMES: Record<WriterEngine, string> = { claude: "Claude", chatgpt: "ChatGPT", gemini: "Gemini" };

export function WriterPicker({ writers, busy, label, onWrite, disabled = false }: { writers: Writer[] | undefined; busy: boolean; label: string; onWrite: (engine: WriterEngine) => void; disabled?: boolean }) {
  const { t, locale } = useI18n();
  const { go } = useShell();
  const ready = (writers ?? []).filter((writer) => writer.ready);
  const [engine, setEngine] = useState<WriterEngine | null>(null);
  if (!writers) return null;
  if (!ready.length) {
    return (
      <span className="writer-picker none">
        <span className="faint small">{t("content.noWriter")}</span>
        <Button small onClick={() => go({ page: "sources" })}>
          <PlugZap size={14} />
          {t("common.goToSources")}
        </Button>
      </span>
    );
  }
  const chosen = ready.find((writer) => writer.engine === engine) ?? ready[0]!;
  return (
    <span className="writer-picker">
      {ready.length > 1 ? (
        <select className="oc-select" value={chosen.engine} aria-label={t("content.writer")} onChange={(event) => setEngine(event.target.value as WriterEngine)}>
          {ready.map((writer) => (
            <option key={writer.engine} value={writer.engine}>
              {NAMES[writer.engine]} · {writer.model}
            </option>
          ))}
        </select>
      ) : (
        <span className="faint small">
          {NAMES[chosen.engine]} · {chosen.model}
        </span>
      )}
      <span className="cost-label" title={t("content.writerCostHint")}>
        ≈ {formatUsd(chosen.usd, locale)}
      </span>
      <Button small primary busy={busy} disabled={disabled} onClick={() => onWrite(chosen.engine)}>
        <PenLine size={14} />
        {label}
      </Button>
    </span>
  );
}
