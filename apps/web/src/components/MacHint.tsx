"use client";

// Under a program's "Download for macOS" button: the apps are not notarized yet, so a Mac visitor
// sees the one command that lets macOS open the app before they download it. Other systems see
// nothing; the full steps are in the card under the program's downloads (`#<id>-macos`).

import { useEffect, useState } from "react";
import { TriangleAlert } from "lucide-react";

import { CopyButton } from "@/components/CopyButton";
import { detectPlatform } from "@/components/DownloadButton";
import { quarantineCommand } from "@/lib/installers";

export interface MacHintLabels {
  title: string;
  text: string;
  more: string;
  copy: string;
  copied: string;
}

export function MacHint({ app, anchor, labels }: { app: string; anchor: string; labels: MacHintLabels }) {
  const [mac, setMac] = useState(false);
  useEffect(() => setMac(detectPlatform() === "macos"), []);
  if (!mac) return null;
  const command = quarantineCommand(app);
  return (
    <div className="mac-hint" role="note">
      <p>
        <TriangleAlert size={16} aria-hidden="true" /> <strong>{labels.title}</strong> {labels.text}
      </p>
      <div className="code-block">
        <pre className="code">
          <code>{command}</code>
        </pre>
        <CopyButton text={command} label={labels.copy} done={labels.copied} />
      </div>
      <a href={`#${anchor}`}>{labels.more} →</a>
    </div>
  );
}
