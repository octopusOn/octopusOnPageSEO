// The grid on a map: OpenFreeMap's vector tiles drawn by MapLibre, each point a coloured pin with
// the business's place there, the business's own point ringed in the middle. Loaded only when the Local SEO page
// shows a map, so the rest of the program stays light. When the map cannot load (no WebGL, no
// network), the page draws the grid without it.

import { useEffect, useRef } from "react";
import { LngLatBounds, Map as MapLibre, Marker, setWorkerUrl } from "maplibre-gl";
import workerUrl from "maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url";
import "maplibre-gl/dist/maplibre-gl.css";
import type { GridCell, GridPoint } from "../../api/types";
import { bandOf, pointLabel } from "./local";

setWorkerUrl(workerUrl);

/** OpenFreeMap's styles: tiles, fonts and sprites all come from this host. */
const STYLES = { dark: "https://tiles.openfreemap.org/styles/dark", light: "https://tiles.openfreemap.org/styles/positron" };

function lightTheme(): boolean {
  const chosen = document.documentElement.dataset.theme;
  return chosen ? chosen === "light" : window.matchMedia("(prefers-color-scheme: light)").matches;
}

export interface GridMapProps {
  /** Points with what they showed, or only where they are (before the first run). */
  cells: (GridCell | GridPoint)[];
  centre: { lat: number; lng: number; title: string };
  /** Selected point (row, col), outlined. */
  selected: string | null;
  onSelect: (key: string) => void;
  /** The map could not be drawn. */
  onFail: () => void;
  label: (cell: GridCell | GridPoint) => string;
}

const keyOf = (cell: GridPoint) => `${cell.row}-${cell.col}`;

export default function GridMap({ cells, centre, selected, onSelect, onFail, label }: GridMapProps) {
  const box = useRef<HTMLDivElement>(null);
  const map = useRef<MapLibre | null>(null);
  const markers = useRef<Marker[]>([]);
  // Callbacks change on every render; the map reads the latest ones.
  const handlers = useRef({ onSelect, onFail, label });
  handlers.current = { onSelect, onFail, label };
  const titleOf = useRef(centre.title);
  titleOf.current = centre.title;

  useEffect(() => {
    if (!box.current) return;
    let created: MapLibre;
    try {
      created = new MapLibre({
        container: box.current,
        style: lightTheme() ? STYLES.light : STYLES.dark,
        center: [centre.lng, centre.lat],
        zoom: 13,
        attributionControl: { compact: true },
        cooperativeGestures: true,
      });
    } catch {
      handlers.current.onFail();
      return;
    }
    created.on("error", (event) => {
      // A style or tile that does not load leaves an empty map: the grid is drawn without it.
      if (!created.isStyleLoaded()) handlers.current.onFail();
      else console.warn(event.error);
    });
    map.current = created;
    return () => {
      markers.current.forEach((marker) => marker.remove());
      markers.current = [];
      created.remove();
      map.current = null;
    };
  }, [centre.lat, centre.lng]);

  useEffect(() => {
    const current = map.current;
    if (!current) return;
    markers.current.forEach((marker) => marker.remove());
    markers.current = [];
    const bounds = new LngLatBounds();
    const middle = (Math.round(Math.sqrt(cells.length)) - 1) / 2;
    for (const cell of cells) {
      const element = document.createElement("button");
      element.type = "button";
      const read = "position" in cell;
      const centre = cell.row === middle && cell.col === middle;
      element.className = `grid-pin ${read ? `band-${bandOf(cell)}` : "band-planned"}${centre ? " centre" : ""}${selected === keyOf(cell) ? " selected" : ""}`;
      element.textContent = read ? pointLabel(cell) : "";
      const text = centre ? `${handlers.current.label(cell)} · ${titleOf.current}` : handlers.current.label(cell);
      element.setAttribute("aria-label", text);
      element.title = text;
      element.addEventListener("click", () => handlers.current.onSelect(keyOf(cell)));
      markers.current.push(new Marker({ element }).setLngLat([cell.lng, cell.lat]).addTo(current));
      bounds.extend([cell.lng, cell.lat]);
    }
    if (!bounds.isEmpty()) current.fitBounds(bounds, { padding: 48, duration: 0, maxZoom: 16 });
  }, [cells, centre.lat, centre.lng, centre.title, selected]);

  return <div ref={box} className="grid-map" />;
}
