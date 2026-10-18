"use client";

import Link from "next/link";
import { useEffect, useRef, useState } from "react";
import { usePathname } from "next/navigation";
import { ChevronDown } from "lucide-react";

import type { ProductCard } from "@/lib/cms";
import { ProgramIcon } from "./ProgramIcon";

/** The header's list of every program, each with its own icon. */
export function ProgramsMenu({ label, programs }: { label: string; programs: ProductCard[] }) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const pathname = usePathname();

  useEffect(() => setOpen(false), [pathname]);
  useEffect(() => {
    if (!open) return undefined;
    const close = (event: MouseEvent) => {
      if (root.current && !root.current.contains(event.target as Node)) setOpen(false);
    };
    const escape = (event: KeyboardEvent) => event.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", escape);
    };
  }, [open]);

  if (programs.length === 0) return null;
  return (
    <div className="programs-menu" ref={root}>
      <button type="button" className="programs-button" aria-expanded={open} onClick={() => setOpen(!open)}>
        {label} <ChevronDown size={15} aria-hidden="true" />
      </button>
      <ul className={`programs-panel ${open ? "open" : ""}`}>
        {programs.map((program) => (
          <li key={program.url}>
            <Link href={program.url} aria-current={pathname === program.url ? "page" : undefined}>
              <ProgramIcon product={program.product} size={36} />
              <span>
                <b>{program.name}</b>
                {program.excerpt && <small>{program.excerpt}</small>}
              </span>
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
}
