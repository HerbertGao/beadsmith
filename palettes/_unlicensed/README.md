# ⚠️ Unlicensed — do NOT ship without resolving

These four domestic-brand palettes (COCO / 漫漫 / 盼盼 / 咪小窝) are **staged for
evaluation only**. Their color values are derived from an **AGPL-3.0** upstream
compilation (`Zippland/perler-beads`, file `src/app/colorSystemMapping.json`).
Two other repos that carry the same data under MIT / Apache-2.0 labels are
byte-for-byte copies of that AGPL file (identical SHA-256) — i.e. ineffective
relicensing, not a clean source.

AGPL-3.0 is a strong copyleft license incompatible with this project's
Apache-2.0 license and its commercial / app-store goals.

**Consequences of shipping these as-is:**

- Would obligate the whole combined work under AGPL-3.0 (source disclosure to
  network users), conflicting with monetization plans.

**These files are therefore:**

- kept out of the top-level `palettes/` set (not auto-discoverable),
- **not** listed in the repository `NOTICE` (no clean-license claim is made).

**To make them shippable**, replace these values with an independently sourced
set — e.g. sample each brand's **official physical color card** to obtain a flat
`code → RGB` list. Individual color facts are not copyrightable; the AGPL
protection attaches to Zippland's cross-reference *compilation*, not to the
colors themselves.
