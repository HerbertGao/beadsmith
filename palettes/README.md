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

## `mard.json`

- **Series:** MARD 拼豆, the classic **solid** color set — series `A`–`H`
  plus `M` (221 colors).
- **Format:** `{ "brand": "MARD", "colors": [ { "code", "name", "rgb": "#RRGGBB" }, ... ] }`.
  `rgb` is `#RRGGBB` computed from the upstream `R,G,B` via `#%02X%02X%02X`.
  `name` equals `code` — MARD beads are sold and referenced by code, not by
  color names.
- **Count:** 221 colors (`A01`–`A26`, `B01`–`B32`, `C01`–`C29`, `D01`–`D26`,
  `E01`–`E24`, `F01`–`F25`, `G01`–`G21`, `H01`–`H23`, `M01`–`M15`).

### Caveats

- **Approximate, unofficial colors.** No manufacturer-published digital color
  spec exists for MARD (a physical bead product). These hex values are
  **community measurements**, intended for nearest-color matching. As a
  confidence check, they were cross-validated against a second independent
  community table and agreed on **221/221** solid colors.
- **Solid subset only.** MARD's full lineup adds ~70 special-effect beads
  (`P`/`Q`/`R`/`T`/`Y`/`ZG` — transparent, glow, glitter). Those are
  **excluded**: a transparent bead's hex does not represent its opaque
  appearance, so including it would produce physically wrong nearest-color
  matches. Add a separate `mard_special.json` if that use case ever arises.

### Source & license

The color data is derived from the
[`maxcleme/beadcolors`](https://github.com/maxcleme/beadcolors) project
(file `raw/mard.csv`), distributed under the **MIT License**
(Copyright (c) 2020 maxcleme). The upstream URL and exact commit hash are
recorded in the repository-root [`NOTICE`](../NOTICE) file.

> **Domestic brands (COCO / 漫漫 / 盼盼 / 咪小窝) — staged, not shippable.**
> The only known digital source is an **AGPL-3.0** compilation, incompatible with
> this project's Apache-2.0 license and commercial goals. Draft palettes derived
> from it are staged under [`_unlicensed/`](./_unlicensed/) **for evaluation
> only** — they are not in the shippable set and not listed in `NOTICE`. See that
> folder's README before using them; they must be re-sourced independently
> (e.g. sampled from official physical color cards) before release.

## Other brand palettes (Artkal A/C/M/R, Hama, Perler, Nabbi, Yant)

Additional bead-brand palettes, all derived the same way from
[`maxcleme/beadcolors`](https://github.com/maxcleme/beadcolors) (**MIT**,
retrieved at commit `29229889`). Same format as above; `name` falls back to
`code` where the upstream table has no color names.

| File | Brand | Colors | Named? |
|---|---|---:|---|
| `artkal_a.json` | Artkal A | 145 | ✓ |
| `artkal_c.json` | Artkal C | 174 | ✓ |
| `artkal_m.json` | Artkal M | 220 | code only |
| `artkal_r.json` | Artkal R | 89 | ✓ |
| `hama.json` | Hama Midi | 92 | ✓ |
| `hama_maxi.json` | Hama Maxi | 25 | ✓ |
| `hama_mini.json` | Hama Mini | 78 | ✓ |
| `nabbi.json` | Nabbi | 30 | ✓ |
| `perler.json` | Perler | 103 | ✓ (SKU-style codes, e.g. `80-15089`) |
| `perler_caps.json` | Perler Caps | 26 | ✓ |
| `perler_mini.json` | Perler Mini | 41 | ✓ |
| `yant.json` | Yant | 119 | ✓ |

### Caveats

- **Approximate, unofficial, community-measured colors** — same status as
  `artkal_s.json`; intended for nearest-color matching.
- **Full sets, not solid-filtered.** Unlike `mard.json` (deliberately limited to
  its 221 solid colors), these palettes include every color in the upstream CSV.
  If a set contains transparent / glow / glitter beads, their hex will not
  represent an opaque appearance and may yield odd matches — filter via
  `--max-colors` or trim the file if this matters for a given brand.
- **Diamond Dotz not included.** The upstream `raw/diamondDotz.csv` is a
  diamond-painting (resin drill) product, a different craft from fusible beads,
  so it is out of scope here; add it the same way if ever wanted.
