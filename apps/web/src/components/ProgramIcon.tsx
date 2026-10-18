import { MARK_PIECES } from "./Logo";
import { findProduct } from "@/lib/site";

/**
 * A program's icon: the white mark on the program's own ground (`data-product`, the same deep
 * color its window uses), so every program is recognisable at a glance.
 */
export function ProgramIcon({ product, size = 48 }: { product: string | null | undefined; size?: number }) {
  const ground = findProduct(product)?.ground ?? "log";
  return (
    <span className="program-icon" data-product={ground} style={{ width: size, height: size }} aria-hidden="true">
      <svg viewBox="0 0 64 64" width={size * 0.62} height={size * 0.62} focusable="false">
        {MARK_PIECES.map((d) => (
          <path key={d} d={d} fill="url(#program-mark-fill)" />
        ))}
      </svg>
    </span>
  );
}

/** The gradient the program icons share; rendered once in the layout. */
export function ProgramIconDefs() {
  return (
    <svg width="0" height="0" style={{ position: "absolute" }} aria-hidden="true" focusable="false">
      <defs>
        <linearGradient id="program-mark-fill" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#ffffff" />
          <stop offset="0.55" stopColor="#eef1f6" />
          <stop offset="1" stopColor="#c3cad8" />
        </linearGradient>
      </defs>
    </svg>
  );
}
