"use client";

import { useEffect, useState } from "react";
import { Download } from "lucide-react";

import { pickInstaller, type Installer } from "@/lib/installers";
import type { Arch, Platform } from "@/lib/releases";

export interface DownloadChoice extends Installer {
  url: string;
  /** Architecture shown on the button when the platform offers several. */
  archLabel: string | null;
}

interface UserAgentData {
  platform?: string;
  getHighEntropyValues?: (hints: string[]) => Promise<{ architecture?: string; bitness?: string }>;
}

function userAgentData(): UserAgentData | undefined {
  return (navigator as Navigator & { userAgentData?: UserAgentData }).userAgentData;
}

function detectPlatform(): Platform | null {
  const hint = `${userAgentData()?.platform ?? ""} ${navigator.userAgent}`.toLowerCase();
  if (hint.includes("android") || hint.includes("iphone") || hint.includes("ipad")) return null;
  if (hint.includes("mac")) return "macos";
  if (hint.includes("win")) return "windows";
  if (hint.includes("linux") || hint.includes("x11")) return "linux";
  return null;
}

/** CPU architecture from Client Hints (Chromium); other browsers keep it private. */
async function detectArch(): Promise<Arch | null> {
  try {
    const values = await userAgentData()?.getHighEntropyValues?.(["architecture", "bitness"]);
    if (values?.architecture === "arm") return values.bitness === "32" ? "arm32" : "arm64";
    if (values?.architecture === "x86" && values.bitness !== "32") return "x64";
  } catch {
    // Hints refused: fall back to the default preference.
  }
  return null;
}

/**
 * Offers the installer for the visitor's operating system, detected in the browser; before
 * detection (and on phones) it links to the download page.
 */
export function DownloadButton({
  choices,
  fallbackHref,
  label,
  genericLabel,
  platformNames,
  className = "button button-primary button-large",
}: {
  choices: DownloadChoice[];
  fallbackHref: string;
  label: string;
  genericLabel: string;
  platformNames: Record<Platform, string>;
  className?: string;
}) {
  const [system, setSystem] = useState<{ platform: Platform | null; arch: Arch | null }>({ platform: null, arch: null });
  useEffect(() => {
    let live = true;
    const platform = detectPlatform();
    setSystem({ platform, arch: null });
    if (platform) void detectArch().then((arch) => live && arch && setSystem({ platform, arch }));
    return () => {
      live = false;
    };
  }, []);

  const { platform, arch } = system;
  const choice = platform ? pickInstaller(choices, platform, arch) : undefined;
  if (!platform || !choice) {
    return (
      <a className={className} href={fallbackHref}>
        <Download size={18} aria-hidden="true" />
        {genericLabel}
      </a>
    );
  }
  const arches = new Set(choices.filter((c) => c.platform === platform).map((c) => c.arch));
  const text = label.replace("{os}", platformNames[platform]);
  return (
    <a className={className} href={choice.url} data-platform={platform} data-arch={choice.arch ?? undefined}>
      <Download size={18} aria-hidden="true" />
      {text}
      {arches.size > 1 && choice.archLabel && <small className="button-detail">{choice.archLabel}</small>}
    </a>
  );
}
