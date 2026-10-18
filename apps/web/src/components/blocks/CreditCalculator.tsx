"use client";

import { useMemo, useState } from "react";

export interface CalculatorRow {
  label: string;
  unit: string;
  credits: number;
  quantity: number;
  max: number;
}

/**
 * Estimates the credits (and dollars) a month of work costs: one slider per paid request type,
 * priced in the CMS. Local work is free, so only API requests appear.
 */
export function CreditCalculator({
  rows,
  creditUsd,
  locale,
  labels,
}: {
  rows: CalculatorRow[];
  creditUsd: number;
  locale: string;
  labels: { total: string; credits: string };
}) {
  const [quantities, setQuantities] = useState(() => rows.map((row) => row.quantity));
  const number = useMemo(() => new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }), [locale]);
  const money = useMemo(() => new Intl.NumberFormat(locale, { style: "currency", currency: "USD", maximumFractionDigits: 2 }), [locale]);
  const totals = rows.map((row, index) => row.credits * (quantities[index] ?? 0));
  const credits = totals.reduce((sum, value) => sum + value, 0);

  const set = (index: number, value: number) => {
    const row = rows[index];
    if (!row) return;
    const clean = Math.max(0, Math.min(row.max || 1_000_000, Number.isFinite(value) ? Math.round(value) : 0));
    setQuantities((current) => current.map((q, i) => (i === index ? clean : q)));
  };

  return (
    <div className="calculator card">
      <ul className="calculator-rows">
        {rows.map((row, index) => (
          <li key={row.label}>
            <div className="calculator-label">
              <b>{row.label}</b>
              <small>
                {number.format(row.credits)} {labels.credits} / {row.unit}
              </small>
            </div>
            <input
              type="range"
              min={0}
              max={row.max || 1000}
              step={row.max > 2000 ? 10 : 1}
              value={quantities[index] ?? 0}
              onChange={(event) => set(index, Number(event.target.value))}
              aria-label={row.label}
            />
            <input
              type="number"
              className="calculator-input"
              min={0}
              max={row.max || undefined}
              value={quantities[index] ?? 0}
              onChange={(event) => set(index, Number(event.target.value))}
              aria-label={`${row.label} (${row.unit})`}
            />
            <span className="calculator-sum">{number.format(totals[index] ?? 0)}</span>
          </li>
        ))}
      </ul>
      <div className="calculator-total" aria-live="polite">
        <span>{labels.total}</span>
        <b>
          {number.format(credits)} {labels.credits}
        </b>
        <span className="muted">≈ {money.format(credits * creditUsd)}</span>
      </div>
    </div>
  );
}
