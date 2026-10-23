// A report page drawn at full width and scaled down to its box, like a sheet seen whole: the
// Reports page's preview and the edit dialog's live preview.

import { useEffect, useRef, useState } from "react";

/** Width the report page is drawn at before it is scaled to fit. */
const PAGE_WIDTH = 980;

/** A page drawn at full width and scaled down to its box, like a sheet seen whole. */
export function ScaledPage({ title, html, dim = false }: { title: string; html: string; dim?: boolean }) {
  const box = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(1);
  useEffect(() => {
    const element = box.current;
    if (!element) return undefined;
    const fit = () => setScale(Math.min(1, element.clientWidth / PAGE_WIDTH));
    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  return (
    <div ref={box} className={`report-preview ${dim ? "dim" : ""}`.trim()}>
      <iframe title={title} sandbox="" srcDoc={html} style={{ width: PAGE_WIDTH, height: `${100 / scale}%`, transform: `scale(${scale})` }} />
    </div>
  );
}
