// The project in the sidebar, opening a list of every project to switch to, and the dashboard.

import { useEffect, useRef, useState } from "react";
import { Check, ChevronsUpDown, LayoutGrid } from "lucide-react";
import type { ProjectRow } from "../../api/types";
import { useI18n } from "../../i18n";

export function ProjectSwitcher({ project, projects, onProject, onAll }: { project: ProjectRow; projects: ProjectRow[]; onProject: (id: number) => void; onAll: () => void }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return undefined;
    const away = (event: MouseEvent) => {
      if (!box.current?.contains(event.target as Node)) setOpen(false);
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("mousedown", away);
      document.removeEventListener("keydown", key);
    };
  }, [open]);
  return (
    <div className="project-switch" ref={box}>
      <button type="button" className="rail-project" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen(!open)} title={t("common.projectSwitch")}>
        <span className="faint">{t("common.project")}</span>
        <strong>{project.name}</strong>
        <small>
          {project.domain} · {project.language}-{project.country}
        </small>
        <ChevronsUpDown size={14} className="project-switch-icon" aria-hidden="true" />
      </button>
      {open && (
        <div className="oc-menu project-switch-menu" role="menu">
          {projects.map((row) => (
            <button
              key={row.id}
              type="button"
              role="menuitem"
              className={`oc-menu-item ${row.id === project.id ? "active" : ""}`}
              onClick={() => {
                setOpen(false);
                onProject(row.id);
              }}
            >
              <span className="project-switch-name">
                <strong>{row.name}</strong>
                <small className="faint">{row.domain}</small>
              </span>
              {row.id === project.id && <Check size={14} aria-hidden="true" />}
            </button>
          ))}
          <button
            type="button"
            role="menuitem"
            className="oc-menu-item"
            onClick={() => {
              setOpen(false);
              onAll();
            }}
          >
            <LayoutGrid size={14} aria-hidden="true" />
            {t("common.allProjects")}
          </button>
        </div>
      )}
    </div>
  );
}
