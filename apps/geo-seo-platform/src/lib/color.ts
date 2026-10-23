// Colours as the report renderer computes them: mixes (its `color-mix`) and the ink that reads
// on a colour.

export const INK = "#0f1726";
export const WHITE = "#ffffff";

const channels = (color: string) => [1, 3, 5].map((at) => parseInt(color.slice(at, at + 2), 16));

/** Whether `value` is a `#rrggbb` colour. */
export function isColor(value: string): boolean {
  return /^#[0-9a-fA-F]{6}$/.test(value);
}

/** `a` mixed with `b`, with `weight` of `a` (0–1), as `#rrggbb`. */
export function mix(a: string, b: string, weight: number): string {
  const [left, right] = [channels(a), channels(b)];
  return `#${left.map((value, index) => Math.round(value * weight + right[index]! * (1 - weight)).toString(16).padStart(2, "0")).join("")}`;
}

/** Ink that reads on `color`: near-black on light colours, white on dark ones. */
export function inkOn(color: string): string {
  const [r, g, b] = channels(color).map((value) => {
    const channel = value / 255;
    return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r! + 0.7152 * g! + 0.0722 * b! > 0.42 ? INK : WHITE;
}
