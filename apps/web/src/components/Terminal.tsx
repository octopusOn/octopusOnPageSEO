"use client";

import { useEffect, useState } from "react";

export interface TerminalStep {
  command: string;
  output: string[];
}

const LOG_SCRIPT: TerminalStep[] = [
  {
    command: "octopuson-log import acme.com /var/log/nginx/",
    output: ["Format       combined (detected)", "Hits         4,812,330 stored · 0 duplicates", "Throughput   768,104 lines/s"],
  },
  {
    command: "octopuson-log query acme.com --template googlebot-errors 'date:last7d'",
    output: [
      "url                      status  hits  last_seen",
      "/old-category/shoes        404   1,204  2026-10-12",
      "/api/cart                  500     311  2026-10-12",
    ],
  },
  {
    command: "octopuson-log robots acme.com",
    output: ["bot           group   blocked  disallowed  verified", "gptbot        gptbot  yes         24,320    24,320", "googlebot     *                       12         0"],
  },
];

/** A terminal replaying a short CLI session; static for visitors who prefer reduced motion. */
export function Terminal({ script = LOG_SCRIPT, label = "octopuson-log command line session", compact = false }: { script?: TerminalStep[]; label?: string; compact?: boolean }) {
  const SCRIPT = script;
  const [step, setStep] = useState(SCRIPT.length);
  const [typed, setTyped] = useState(0);

  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return undefined;
    setStep(0);
    setTyped(0);
    return undefined;
  }, []);

  useEffect(() => {
    if (step >= SCRIPT.length) return undefined;
    const command = SCRIPT[step]?.command ?? "";
    const delay = typed < command.length ? 28 : 1200;
    const timer = window.setTimeout(() => {
      if (typed < command.length) setTyped(typed + 1);
      else {
        setStep(step + 1);
        setTyped(0);
      }
    }, delay);
    return () => window.clearTimeout(timer);
  }, [step, typed, SCRIPT]);

  return (
    <div className={`terminal ${compact ? "compact" : ""}`} role="img" aria-label={label}>
      <div className="terminal-bar">
        <span />
        <span />
        <span />
      </div>
      <pre>
        {SCRIPT.map((entry, index) => {
          if (index > step) return null;
          const current = index === step;
          const command = current ? entry.command.slice(0, typed) : entry.command;
          return (
            <span key={entry.command}>
              <span className="prompt">$ </span>
              {command}
              {current && <span className="caret" />}
              {"\n"}
              {!current && entry.output.map((line, i) => <span key={`${i}${line}`} className="out">{line + "\n"}</span>)}
            </span>
          );
        })}
      </pre>
    </div>
  );
}
