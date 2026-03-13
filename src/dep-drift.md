# Dependency Drift

A single-file HTML tool that visualizes how Gradle dependencies evolve across
tagged releases. No build step, no server — open `dep-drift.html` in a browser.

## Problem

Gradle projects using version catalogs (`libs.versions.toml`) accumulate
dependency changes across releases: bumps, additions, removals. There's no
built-in way to see the full timeline. You end up doing `git diff v0.0.3..v0.0.5
-- gradle/libs.versions.toml` repeatedly and trying to hold the picture in your
head.

This tool gives you a single matrix view: rows are dependencies, columns are
releases. Every cell is classified by change type and semver severity. You can
compare any two releases, sort by impact, and immediately see what matters.

## How it works (current state)

- **Self-contained HTML** — all CSS and JS inline, only external dependency is
  Google Fonts (DM Serif Display, DM Sans, IBM Plex Mono)
- **Hardcoded sample data** — 15 realistic Gradle/Spring Boot dependencies
  across 6 tags (v0.0.1 through v0.0.6). Replace the `TAGS` and `DEPS` arrays
  in the `<script>` block to use your own data.
- **No build step** — open the file, it works

## Features

### 1. Semver-aware severity classification

Every version bump is parsed into major/minor/patch. The visual weight scales
with impact:

- **Major** (e.g. kotlin 1.9.23 → 2.0.0): strong red-orange, bold,
  `MAJOR` badge. These are the ones you need to read the changelog for.
- **Minor** (e.g. spring-boot 3.2.0 → 3.3.0): amber, `minor` badge.
  Likely safe but worth scanning.
- **Patch** (e.g. postgresql 42.7.1 → 42.7.2): muted gold, no badge.
  Probably fine.

Rationale: the old version treated all bumps the same. A patch on logback
and a major Kotlin bump had equal visual weight, which is useless.

### 2. Release comparison picker

Two `<select>` dropdowns: "From" and "To". When you select a sub-range
(e.g. v0.0.2 → v0.0.4), only those columns are shown. Rows with zero
changes in that range auto-hide when "Only show changed" is enabled.

This is the core UX improvement — it answers "what changed between these
two releases?" directly instead of making you visually scan.

A "Show all releases" button resets to the full timeline.

### 3. Diff summary panel

Appears below the compare bar when in compare mode. Shows a prose breakdown
with chips: `2 major — kotlin, jackson · 3 minor — spring-boot, micrometer...`
etc. This is the TL;DR for a release range.

### 4. Per-column release summaries

Under each tag header, a compact line like `1M 2m 1p 1+` shows how many
majors, minors, patches, and additions happened in that release. Lets you
instantly spot which release was heavy (e.g. "v0.0.3 had 2 majors").

### 5. Changelog links

Hover any bumped/added cell to reveal an external-link icon. Clicks through to
the dependency's GitHub releases page. Each dep in the `DEPS` array has a
`repoUrl` field — this generates the link.

Rationale: seeing "jackson 2.16.0 → 2.17.0" is information, but being one
click away from the release notes is actionable.

### 6. Staleness indicator

Rightmost column shows when each dep was last changed:
- `logback`: "Never changed / 6 releases pinned" — in a warning color
- `opentelemetry`: "v0.0.6" (changed in latest) — in a fresh/green tint
- `resilience4j`: "v0.0.6 / 0 releases ago"

Rationale: staleness is a signal. A dep pinned for many releases might be
intentionally frozen, abandoned upstream, or simply forgotten. Either way
you should know.

### 7. Sort by impact

Dropdown with 4 modes:
- **Impact** (default): weighted score — major bumps worth 10, minor 3,
  patch 1, added 5, removed 2. Most-changed deps float to top.
- **Name**: alphabetical
- **Staleness**: most stale first
- **Category**: grouped by category, then by impact within each group

### 8. Other features

- **Search/filter**: text input filters rows by dep name or category.
  Press `/` to focus, `Esc` to clear.
- **"Only show changed" toggle**: hides rows with no changes (or no changes
  in the current compare range).
- **Click-to-highlight**: click any row to pin a highlight on it. Useful for
  tracking a specific dep across many columns.

## Design decisions

### Aesthetic: "Engineering Logbook"

Warm paper background with subtle SVG noise grain. Not a dashboard — more like
a carefully typeset changelog. The table is the hero; no unnecessary chrome.

### Typography

- **DM Serif Display** for the page title — distinctive, not generic
- **DM Sans** for body text and UI controls — clean, good weight range
- **IBM Plex Mono** for version numbers and tag headers — readable at small
  sizes, more character than Fira Code

### Color system

Entire palette in OKLCH for perceptual uniformity. Severity colors are
intentionally distinct from each other even at small sizes:
- Major: `oklch(62% 0.19 25)` — warm red-orange
- Minor: `oklch(68% 0.15 70)` — amber
- Patch: `oklch(72% 0.06 70)` — muted gold
- Added: `oklch(62% 0.15 185)` — teal
- Removed: `oklch(62% 0.14 15)` — rose

### Modern CSS features used

| Feature | Purpose |
|---------|---------|
| `@layer` | Organize reset, base, components, utilities |
| `@property` | Animatable `--row-highlight` for hover transitions |
| OKLCH colors | Entire palette in perceptually uniform color space |
| Container queries | Table compacts at narrow widths without viewport media queries |
| `text-wrap: balance` | Heading text wrapping |
| `text-box: trim-both` | Optical vertical alignment on heading |
| Scroll-driven animations | Rows fade in on scroll (`animation-timeline: view()`) |
| `field-sizing: content` | Search input grows with content |
| `color-mix()` | Row hover background blending |
| `clamp()` | Fluid typography |
| `prefers-reduced-motion` | Respects user preference, disables all animation |
| `scrollbar-color` | Themed scrollbar matching palette |
| `linear()` easing | Spring easing curve for interactions |

## Not yet built

### CLI data extraction script

A script you run from a Gradle project's git root:

```
dep-drift-extract [--tag-pattern "v*"] [--output deps.json]
```

What it would do:
1. List all git tags matching the pattern, sorted chronologically
2. For each tag, run `git show <tag>:gradle/libs.versions.toml` (no checkout —
   fast and non-destructive)
3. Parse the TOML `[versions]` and `[libraries]` sections
4. Output a `deps.json` file

Language: Python (TOML parsing built-in since 3.11 via `tomllib`).

### JSON import in the HTML viewer

Instead of editing the `DEPS` array in the HTML source, drag-and-drop a
`deps.json` file onto the page (or use a file picker).

### Auto-resolve changelog URLs

Given Maven coordinates (group + artifact), auto-generate links to:
- Maven Central version page
- GitHub releases (resolved via Maven POM `scm` URL)

### Dark mode

The OKLCH palette is structured to support `light-dark()` easily. Would need
a second set of `--bg-*` and `--text-*` values and a toggle.

### `build.gradle.kts` parsing fallback

For projects not using version catalogs. Much harder — versions are scattered
across build scripts, sometimes in variables, sometimes inline. Low priority.

## Data format

The CLI script (when built) will output this JSON:

```json
{
  "tags": ["v0.0.1", "v0.0.2", "v0.0.3"],
  "dependencies": [
    {
      "name": "spring-boot",
      "category": "framework",
      "repoUrl": "https://github.com/spring-projects/spring-boot",
      "versions": ["3.2.0", "3.2.0", "3.3.0"]
    }
  ]
}
```

- `versions` array is positional — index 0 corresponds to `tags[0]`
- `null` means the dep wasn't present in that tag
- `category` and `repoUrl` are optional but improve the UI

## Files

- `dep-drift.html` — the viewer (self-contained, ~600 lines)
- `dep-drift.md` — this spec
