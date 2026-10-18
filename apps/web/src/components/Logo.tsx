/** The three blocks of the OctopusOn SEO mark, in a 64-unit square. */
export const MARK_PIECES = [
  "M31.95 3.93 L46.21 12.70 L46.21 28.72 L60.73 37.39 L49.34 43.59 L34.02 34.57 L34.07 18.85 L20.96 10.63 Z",
  "M17.69 12.39 L29.83 19.80 L29.83 34.67 L13.65 44.25 L13.71 59.01 L1.81 51.66 L1.86 38.05 L17.69 28.47 Z",
  "M31.95 38.10 L48.48 47.98 L62.19 40.27 L62.19 51.66 L48.08 60.07 L31.90 50.80 L16.28 59.62 L16.33 47.27 Z",
];

/** The OctopusOn SEO mark: three white glass blocks turning around a centre. */
export function LogoMark({ size = 32 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true" focusable="false">
      <defs>
        <linearGradient id="logo-mark-fill" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#ffffff" />
          <stop offset="0.55" stopColor="#eef1f6" />
          <stop offset="1" stopColor="#c3cad8" />
        </linearGradient>
        <linearGradient id="logo-mark-sheen" x1="0" y1="1" x2="1" y2="0">
          <stop offset="0.3" stopColor="#ffffff" stopOpacity="0" />
          <stop offset="0.5" stopColor="#ffffff" stopOpacity="0.55" />
          <stop offset="0.7" stopColor="#ffffff" stopOpacity="0" />
        </linearGradient>
      </defs>
      {MARK_PIECES.map((d) => (
        <g key={d}>
          <path d={d} fill="url(#logo-mark-fill)" />
          <path d={d} fill="url(#logo-mark-sheen)" />
        </g>
      ))}
    </svg>
  );
}

export function Logo() {
  return (
    <span className="logo">
      <LogoMark />
      <span className="logo-word">
        OctopusOn <strong>SEO</strong>
      </span>
    </span>
  );
}
