# Palettes

Bundled bead-color palettes loaded by `bead-core`.

## `artkal_s.json`

- **Series:** Artkal **5mm Midi "S"** beads.
- **Format:** `{ "brand": "Artkal S", "colors": [ { "code", "name", "rgb": "#RRGGBB" }, ... ] }`.
  `rgb` is a `#RRGGBB` hex string computed from the upstream `R,G,B` values
  via `#%02X%02X%02X`.
- **Count:** 199 colors. This covers the 159 codes of the core `S01`–`S159`
  range plus 40 additional community-contributed codes (`SE`/`SG`/`SL`/`SN`/`SP`/`ST`).

### Caveats

- **Approximate, unofficial colors.** The hex values are **community
  approximations**, not official Artkal color specifications. They are
  intended for nearest-color matching, where small deviations are tolerable.
- **Not the complete official lineup.** Artkal's official "S" series now spans
  roughly 200+ colors; the core machine-readable set here is the 159-color
  `S` range (with community extras). No clean, openly licensed source for the
  full official table exists, so it is not reproduced here.

### Source & license

The color data is derived from the
[`maxcleme/beadcolors`](https://github.com/maxcleme/beadcolors) project
(file `raw/artkal_s.csv`), distributed under the **MIT License**
(Copyright (c) 2020 maxcleme). The upstream URL and the exact commit hash used
to generate `artkal_s.json` are recorded in the repository-root
[`NOTICE`](../NOTICE) file, which also reproduces the MIT attribution required
for redistribution.
