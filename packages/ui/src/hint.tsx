// A small "i" that explains a number or a column: shown on hover and on keyboard focus, drawn on
// the document body so tables and panels that scroll cannot cut it off.

import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Info } from "lucide-react";

/** Widest tooltip, pixels. */
const WIDTH = 280;

export function Tip({ text, label }: { text: string; label: string }) {
  const id = useId();
  const button = useRef<HTMLButtonElement>(null);
  const bubble = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [place, setPlace] = useState({ top: -9999, left: -9999 });
  useLayoutEffect(() => {
    if (!open || !button.current) return;
    const box = button.current.getBoundingClientRect();
    const height = bubble.current?.offsetHeight ?? 60;
    const width = Math.min(WIDTH, window.innerWidth - 16);
    const above = box.top - 8 - height > 8;
    setPlace({
      top: above ? box.top - 8 - height : box.bottom + 8,
      left: Math.min(Math.max(8, box.left + box.width / 2 - width / 2), window.innerWidth - width - 8),
    });
  }, [open]);
  useEffect(() => {
    if (!open) return undefined;
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    const hide = () => setOpen(false);
    document.addEventListener("keydown", key);
    window.addEventListener("scroll", hide, true);
    return () => {
      document.removeEventListener("keydown", key);
      window.removeEventListener("scroll", hide, true);
    };
  }, [open]);
  return (
    <>
      <button
        ref={button}
        type="button"
        className="ui-tip"
        aria-label={label}
        aria-describedby={open ? id : undefined}
        onMouseEnter={() => setOpen(true)}
        onMouseLeave={() => setOpen(false)}
        onFocus={() => setOpen(true)}
        onBlur={() => setOpen(false)}
        onClick={(event) => {
          event.stopPropagation();
          setOpen(!open);
        }}
      >
        <Info size={12} aria-hidden="true" />
      </button>
      {open &&
        createPortal(
          <div ref={bubble} id={id} role="tooltip" className="ui-tip-bubble" style={{ top: place.top, left: place.left, maxWidth: Math.min(WIDTH, window.innerWidth - 16) }}>
            {text}
          </div>,
          document.body,
        )}
    </>
  );
}
