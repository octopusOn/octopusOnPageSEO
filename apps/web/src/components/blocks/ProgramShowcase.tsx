"use client";

import { useState } from "react";
import { AppWindow, SquareTerminal } from "lucide-react";

import type { MediaFile } from "@/lib/cms";
import { Terminal, type TerminalStep } from "@/components/Terminal";

/**
 * A program's hero visual: its window (the screenshot) floating on the program's ground. The
 * command line waits behind a second tab, so the app comes first.
 */
export function ProgramShowcase({
  name,
  image,
  commands,
  cli,
  labels,
}: {
  name: string;
  image: MediaFile | null;
  commands: string;
  cli: string;
  labels: { app: string; cli: string };
}) {
  const [tab, setTab] = useState<"app" | "cli">("app");
  const script: TerminalStep[] = commands
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((command) => ({ command, output: ["✓ done"] }));
  return (
    <div className="showcase">
      <div className="showcase-glow" aria-hidden="true" />
      {script.length > 0 && (
        <div className="showcase-tabs" role="tablist" aria-label={name}>
          <button type="button" role="tab" aria-selected={tab === "app"} className={tab === "app" ? "active" : ""} onClick={() => setTab("app")}>
            <AppWindow size={15} aria-hidden="true" /> {labels.app}
          </button>
          <button type="button" role="tab" aria-selected={tab === "cli"} className={tab === "cli" ? "active" : ""} onClick={() => setTab("cli")}>
            <SquareTerminal size={15} aria-hidden="true" /> {labels.cli}
          </button>
        </div>
      )}
      <div className="showcase-window" role="tabpanel">
        <div className="showcase-bar" aria-hidden="true">
          <span />
          <span />
          <span />
          <em>{tab === "app" ? name : cli}</em>
        </div>
        {tab === "app" ? (
          image && (
            // Served by the CMS at its final size.
            // eslint-disable-next-line @next/next/no-img-element
            <img src={image.url} alt={image.alt ?? name} width={image.width ?? undefined} height={image.height ?? undefined} />
          )
        ) : (
          <div className="showcase-cli">
            <Terminal script={script} label={`${cli} command line session`} compact />
          </div>
        )}
        {tab === "app" && <span className="showcase-sweep" aria-hidden="true" />}
      </div>
    </div>
  );
}
