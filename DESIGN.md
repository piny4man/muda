---
name: muda
description: Browser-first JPEG/PNG/WebP metadata eraser. Brutalist stamp, acid lime accent.
colors:
  paper: "oklch(0.96 0.012 125)"
  night: "oklch(0.18 0.02 125)"
  ink-light: "oklch(0.22 0.025 130)"
  ink-dark: "oklch(0.93 0.02 125)"
  muted-light: "oklch(0.42 0.02 130)"
  muted-dark: "oklch(0.75 0.02 125)"
  lime-light: "oklch(0.78 0.21 125)"
  lime-dark: "oklch(0.86 0.22 125)"
  bad-light: "oklch(0.52 0.2 25)"
  bad-dark: "oklch(0.72 0.18 25)"
typography:
  display:
    fontFamily: "IBM Plex Sans, ui-sans-serif, system-ui, sans-serif"
    fontSize: "1.75rem"
    fontWeight: 700
    lineHeight: 1.15
    letterSpacing: "normal"
  title:
    fontFamily: "IBM Plex Sans, ui-sans-serif, system-ui, sans-serif"
    fontSize: "1.25rem"
    fontWeight: 700
    lineHeight: 1
    letterSpacing: "0.12em"
  body:
    fontFamily: "IBM Plex Sans, ui-sans-serif, system-ui, sans-serif"
    fontSize: "1rem"
    fontWeight: 400
    lineHeight: 1.45
    letterSpacing: "normal"
  label:
    fontFamily: "IBM Plex Sans, ui-sans-serif, system-ui, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 700
    lineHeight: 1.2
    letterSpacing: "0.08em"
  mono:
    fontFamily: "IBM Plex Mono, ui-monospace, monospace"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: 1.4
    letterSpacing: "normal"
rounded:
  none: "0px"
spacing:
  sm: "8px"
  md: "16px"
  lg: "24px"
  xl: "40px"
components:
  button-primary:
    backgroundColor: "{colors.lime-light}"
    textColor: "{colors.ink-light}"
    rounded: "{rounded.none}"
    padding: "10px 14px"
    height: "44px"
  button-secondary:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink-light}"
    rounded: "{rounded.none}"
    padding: "10px 14px"
    height: "44px"
  stamp:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink-light}"
    rounded: "{rounded.none}"
    padding: "2px 6px"
    typography: "{typography.mono}"
---

# Design System: muda

## 1. Overview

**Creative North Star: "The Cut Stamp"**

A single-purpose tool that should feel like a shop stamp hitting paper: hard rules, no radius, one acid-lime mark. The user is stripping GPS from a photo on a phone. The chrome is a form, not a product tour.

The system rejects warm serif privacy-wellness, forest-green privacy chrome, rounded SaaS pills, glass, Inter, neo-brutalist candy offset shadows, and any third-party font request.

**Key Characteristics:**

- Acid lime on the logo and primary Strip actions only
- 2px ink rules, 0 radius, no drop shadows
- Self-hosted IBM Plex Sans + Mono
- Light paper and dark night via `prefers-color-scheme`
- Mobile-first, 44px (48px coarse) targets, sticky Strip-all on small screens

## 2. Colors

One accent family. Neutrals tint toward hue 125.

### Primary
- **Acid lime** (`oklch(0.78 0.21 125)` light / `oklch(0.86 0.22 125)` dark): logo mark, `Strip` / `Strip all` fills, focus ring, drag-over dropzone. Never body text.

### Neutral
- **Paper** (`oklch(0.96 0.012 125)`): light page.
- **Night** (`oklch(0.18 0.02 125)`): dark page.
- **Ink** (`oklch(0.22 0.025 130)` light / `oklch(0.93 0.02 125)` dark): type and rules.
- **Muted** (`oklch(0.42 0.02 130)` light / `oklch(0.75 0.02 125)` dark): secondary copy, still AA.

### Semantic
- **Bad** (`oklch(0.52 0.2 25)` light / `oklch(0.72 0.18 25)` dark): errors and unsupported.

### Named Rules
**The Fill Rule.** Lime is a fill, not a text color. Type on lime is near-black (`--accent-ink`). Logo geometry is solid bars, not hairline strokes.

**The One Cut Rule.** Accent appears on the mark, primary actions, focus, and drag-over. If lime is everywhere, it is no longer a stamp.

## 3. Typography

**Display Font:** IBM Plex Sans (ui-sans-serif, system-ui)
**Body Font:** IBM Plex Sans
**Label/Mono Font:** IBM Plex Mono

**Character:** Industrial grotesque plus a mono for filenames and tag lists. Self-hosted `woff2` only.

### Hierarchy
- **Headline** (700, 1.75rem, 1.15): page title "Metadata eraser".
- **Title** (700, 1.25rem, 0.12em caps): `MUDA` wordmark.
- **Body** (400, 1rem, 1.45): claims, footer, reports.
- **Label** (700, 0.875rem, 0.08em uppercase): buttons.
- **Mono** (400, 0.75–0.875rem): filenames, sizes, stamps, removed tags. Tabular nums on sizes.

### Named Rules
**The No Phone-Home Rule.** Fonts load from `/fonts/*.woff2`. Google Fonts and CDNs are forbidden.

## 4. Elevation

Flat. Depth is a 2px ink rule, not a shadow. Drag-over inverts the dropzone to lime. Active buttons translate 2px unless `prefers-reduced-motion`.

### Named Rules
**The No-Shadow Rule.** No `box-shadow`. No 6px candy offsets. No glass.

## 5. Components

### Buttons
- **Shape:** square corners (0). 2px ink border. Min height 44px (48px on coarse pointer).
- **Primary:** lime fill, near-black type, uppercase.
- **Secondary / ghost:** paper or transparent, ink type, same border.
- **Hover (fine pointer only):** ink fill, paper or lime type.
- **Active:** `translate(2px, 2px)`.
- **Focus:** 2px lime outline, 2px offset.

### Stamps (status / format)
- Boxed labels, not pills. Mono, uppercase. Done = lime fill. Error = bad fill.

### Dropzone
- Hard rectangle, 2px rule, min height ~8.5rem. Dragging fills lime.

### File row
- 2px ruled block. Square thumb. Actions three equal columns on small screens, row on 640px+.

### Sticky Strip-all
- Fixed to the bottom on small screens when the queue is non-empty, above the home indicator. Hidden from 640px up; the toolbar primary takes over.

### Logo
- Cut-stamp: square frame + diagonal slash, `currentColor`, transparent interior. Wordmark is HTML `MUDA`, not outlined in the SVG.

## 6. Do's and Don'ts

### Do:
- **Do** keep lime rare: logo, Strip, focus, drag-over.
- **Do** use 2px full borders and 0 radius.
- **Do** self-host IBM Plex and keep the privacy claim intact.
- **Do** give every action a 44px target; 48px when `pointer: coarse`.
- **Do** ship both themes through `prefers-color-scheme`, not a custom toggle.

### Don't:
- **Don't** use warm serif privacy-wellness.
- **Don't** use forest-green / teal "privacy tool" chrome.
- **Don't** use rounded SaaS pills, glass, or soft shadows.
- **Don't** default to Inter.
- **Don't** add neo-brutalist candy cards with chunky offset shadows.
- **Don't** load Google Fonts or any third-party request on the page.
- **Don't** set body or link text in lime on paper.
- **Don't** use `#000` or `#fff`; keep the lime-tinted neutrals.
