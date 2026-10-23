import { describe, expect, it } from "vitest";
import type { SnapshotKpi, SnapshotRow, SnapshotSetup } from "../../api/types";
import { chosenCompetitors, defaultPair, deltaText, finishedOf, formatKpi, pathOf, placesText, toggled, toneOf } from "./snapshot";

const kpi = (key: string, before: number | null, after: number | null, higherBetter = true): SnapshotKpi => {
  const change = before !== null && after !== null ? after - before : null;
  return { key, group: "organic", before, after, change, percent: change !== null && before ? (change / before) * 100 : null, higher_better: higherBetter };
};

const row = (id: number, status: SnapshotRow["status"]): SnapshotRow => ({
  id,
  project_id: 1,
  label: null,
  kind: "manual",
  status,
  taken_at: `2026-0${id}-01T09:00:00Z`,
  finished_at: null,
  cost_usd: 0,
  error: null,
  headline: null,
});

const setup = (rows: SnapshotRow[], baseline: number | null): SnapshotSetup => ({
  settings: { schedule: "monthly", competitors: ["b.com", "gone.com", "a.com"], last_run_at: null },
  competitors: ["a.com", "b.com"],
  snapshots: rows,
  baseline_id: baseline,
  next_at: null,
});

describe("snapshots", () => {
  it("writes each number its own way and how it moved", () => {
    expect(formatKpi("traffic", 1525, "tr-TR")).toBe("1.525");
    expect(formatKpi("mention_rate", 35, "en-GB")).toBe("35%");
    expect(formatKpi("health", 84.25, "en-GB")).toBe("84.3");
    expect(formatKpi("traffic_value", null, "en-GB")).toBe("—");
    expect(deltaText(kpi("traffic", 1240, 1525), "tr-TR")).toBe("+%23");
    expect(deltaText(kpi("traffic", 1000, 1050), "en-GB")).toBe("+5%");
    expect(deltaText(kpi("health", 71, 84), "en-GB")).toBe("+13");
    expect(deltaText(kpi("keywords", 0, 12), "en-GB")).toBe("+12");
    expect(deltaText(kpi("traffic", 10, 10), "en-GB")).toBeNull();
    expect(toneOf(kpi("average_position", 18, 12, false))).toBe("good");
    expect(toneOf(kpi("issues", 30, 40, false))).toBe("bad");
    expect(toneOf(kpi("traffic", 5, 5))).toBeUndefined();
    expect(placesText(6)).toBe("+6");
    expect(placesText(-2)).toBe("-2");
    expect(placesText(null)).toBe("");
    expect(pathOf("https://ornek.com/spor?renk=mavi")).toBe("/spor?renk=mavi");
  });

  it("compares the starting point with the latest by default", () => {
    const rows = [row(4, "done"), row(3, "failed"), row(2, "partial"), row(1, "done")];
    expect(finishedOf(rows).map((entry) => entry.id)).toEqual([4, 2, 1]);
    expect(defaultPair(setup(rows, 1))).toEqual({ from: 1, to: 4 });
    expect(defaultPair(setup(rows, null))).toEqual({ from: 1, to: 4 });
    expect(defaultPair(setup([row(1, "done"), row(2, "running")], 1))).toBeNull();
  });

  it("keeps only the project's competitors, in its order", () => {
    const known = setup([], null);
    expect(chosenCompetitors(known)).toEqual(["a.com", "b.com"]);
    expect(toggled(["b.com"], "a.com", known.competitors)).toEqual(["a.com", "b.com"]);
    expect(toggled(["b.com"], "b.com", known.competitors)).toEqual([]);
    expect(toggled(["a.com", "b.com"], "a.com", known.competitors)).toEqual(["b.com"]);
  });
});
