// A small line of a series, drawn as SVG: no axes, the last point marked.

import type { DatedValue } from "../api/types";

export function Sparkline({ points, label, invert = false }: { points: readonly DatedValue[]; label: string; invert?: boolean }) {
  const values = points.map((point) => point.value).filter((value): value is number => value !== null);
  if (values.length < 2) return null;
  const width = 120;
  const height = 32;
  const low = Math.min(...values);
  const high = Math.max(...values);
  const span = high - low || 1;
  const step = width / (values.length - 1);
  const y = (value: number) => {
    const share = (value - low) / span;
    return 3 + (invert ? share : 1 - share) * (height - 6);
  };
  const path = values.map((value, index) => `${index === 0 ? "M" : "L"}${(index * step).toFixed(1)},${y(value).toFixed(1)}`).join(" ");
  const last = values[values.length - 1]!;
  return (
    <svg className="sparkline" viewBox={`0 0 ${width} ${height}`} width={width} height={height} role="img" aria-label={label}>
      <path d={path} fill="none" />
      <circle cx={width} cy={y(last)} r="2.5" />
    </svg>
  );
}
