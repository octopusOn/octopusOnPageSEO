# ADR 0008 — Chromatophore, a shared design language

- Status: accepted
- Date: 2026-10-16

## Context
octopusSEO will ship several programs (the Log Analyser now, the SEO Spider next) plus the website. The desktop UI had its own ad-hoc CSS variables and the website another set; colors, type and component shapes drifted, and nothing could be reused by a second program. The look should feel like a premium native Mac app without decoration competing with the data.

## Decision
One CSS-first package, `packages/design` (`@octopusseo/design`), owns the design language:

- `tokens.json` is the single source of every value (colors per theme, type styles, spacing, radii, shadows, z-index), in the format of the octopusSEO Design System page so the same file documents and drives the system. `scripts/build-tokens.mjs` compiles it to `tokens.css`; CI fails when the two differ.
- A premium Mac-app direction ("Deep Sea"): the product's deep color fills the whole window as one ground (Log Analyser violet, Spider teal), with translucent glass cards, large white titles, white primary buttons and small glossy icon tiles as the only multicolored elements. One theme; native controls are dark.
- The system face comes first (SF Pro on macOS, Segoe UI on Windows) with Geist and Geist Mono bundled as WOFF2 fallbacks (Latin and Latin Extended), so every platform renders offline without a font CDN.
- On macOS the window uses an overlay title bar; the sidebar lists the modules and switches sites from a menu.
- `base.css` and `components.css` provide framework-agnostic `oc-*` component classes (shell, rail, page header, panels, stat tiles, buttons, inputs, tables, loaders, overlays). React apps keep thin wrappers (`components/ui.tsx`); app-specific layout stays in the app.
- The chart palette (`chart-1` … `chart-8`) is a fixed order validated for color-blind separation and contrast against both themes' panel surfaces; canvas charts read it from the root at runtime and redraw on theme changes. Status classes use the status colors, never series colors.

## Alternatives considered
- A React component library: would bind future programs and the website to one framework and add a build step; CSS classes serve React, Next.js and plain HTML alike.
- Tailwind with a shared preset: utility classes would spread the language across markup instead of naming components, and the desktop app has no Tailwind today.
- Fonts from a CDN or system fonts only: a CDN breaks offline desktop use; system fonts lose the identity.

## Consequences
- New programs import `@octopusseo/design`, set `data-product`, and build their screens from `oc-*` classes; a new product hue is a token override in `components.css`.
- Changing a value means editing `tokens.json` and running the package's `build`; the Design System page is refreshed from the same file.
- The website aliases its historical variable names to the shared tokens and sits on the same ground; terminal and code samples stay dark.
