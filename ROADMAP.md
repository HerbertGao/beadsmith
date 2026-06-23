# Beadsmith Roadmap

**End goal:** a reusable Rust core (`bead-core`) wrapped in a Flutter shell,
shipped as native apps on the App Store and Google Play.

Milestones are ordered so each one is independently testable and builds on the
last. The Rust engine is fully validated through the CLI (M0–M7) *before* any
Flutter work starts (M8–M9). Phase 1 = M0–M7, Phase 2 = M8–M9.

```text
M0 ─ M1 ─ M2 ─ M3 ─ M4 ─ M5 ─ M6 ─ M7        (Rust core, CLI-validated)
                                    └─ M8 ─ M9  (Flutter mobile)
```

---

## M0 — Cargo Workspace Init

**Goal:** a building workspace with empty crates wired together.

- Root `Cargo.toml` workspace with `crates/bead-core` and `crates/bead-cli`.
- `bead-core` compiles as a library; `bead-cli` depends on it and runs.
- Add `thiserror` to core, `clap` to cli, `anyhow` to cli.
- `git init` + `.gitignore` (`/target`).

**Done when:** `cargo build` and `cargo test` pass on an empty `lib.rs` +
`main.rs`.

> `bead-ffi` is deferred to M8 — no point in an empty bridge crate until there
> is something to bridge.

---

## M1 — Palette Loader

**Goal:** load and validate external JSON palettes.

- `palette` module: `load_palette`, `validate_palette`.
- `PaletteColor` model (`code`, `name`, `rgb: [u8; 3]`); parse `"#RRGGBB"` →
  `[u8; 3]`.
- Validation: non-empty colors, unique codes, valid hex.
- Ship one real palette JSON under `palettes/` (Artkal S) for testing.

**Done when:** unit tests cover a valid palette, a malformed hex, and an empty
palette; loading the bundled Artkal palette succeeds.

---

## M2 — Resize + Pattern Grid

**Goal:** turn an image into a bead grid (1 pixel = 1 bead), no color matching
yet.

- `image` module: `decode_image`, `crop_center`, `resize_image` (PNG/JPG/JPEG/
  WEBP in).
- `models`: `BeadCell`, `BeadPattern` (cells carry raw resized colors for now).
- Center-crop to target aspect ratio, then resize to `width × height`.

**Done when:** decoding + center-crop + resize to e.g. 80×100 produces a
`BeadPattern` with exactly `width × height` cells; a golden-ish fixture
confirms deterministic output.

---

## M3 — Color Matching

**Goal:** map each cell to the nearest real bead color (Phase 1 = RGB).

- `matcher` module: `ColorMatcher` trait + `find_best_match`.
- Phase 1 implementation: RGB Euclidean distance against the loaded palette.
- `BeadCell.color_index` now points into the palette.

**Done when:** known colors map to their exact palette entry; an off-palette
color maps to the nearest one (asserted in a unit test). Deterministic.

---

## M4 — Statistics

**Goal:** count beads per color from the pattern.

- `statistics` module: `count_colors`, `total_beads`, `generate_summary`.
- `ColorStat` model; `BeadPattern.stats` populated.
- Summary text in the INIT.md format (copyable, no CSV).

**Done when:** counts derive **only** from `BeadPattern` (never from rendered
images); totals equal `width × height`; summary string matches the spec
format.

---

## M5 — Preview Renderer

**Goal:** render the pattern to images.

- `renderer` module: `render_preview` (→ `preview.png`, no coordinates) and
  `render_grid` (→ `grid.png`, row/column numbers + grid lines).
- Both render from `BeadPattern` + palette.

**Done when:** preview and grid PNGs are produced and are byte-identical across
runs for the same input.

---

## M6 — CLI

**Goal:** the full pipeline behind one command.

- `pipeline` module: `generate_pattern` orchestrates load → resize → match →
  stats → render.
- `bead-cli generate --input --palette --width --height --output`.
- Writes `preview.png`, `grid.png`, `pattern.json`, `summary.txt`.
- Stub future subcommands as needed (`palette list/validate`, `inspect`).

**Done when:** the example command from INIT.md runs end-to-end and writes all
four output files.

---

## M7 — Golden Tests

**Goal:** lock output stability.

- Fixed sample image + fixed palette + fixed settings under `tests/golden/`.
- Assert `pattern.json` and `summary.txt` (and optionally `preview.png`) never
  change.
- Add Criterion benchmarks (40×40, 80×100, 100×100, 150×150, 300×300).

**Done when:** golden tests pass in CI; a deliberate algorithm change makes
them fail loudly. **Phase 1 engine is now frozen and trustworthy.**

---

## M8 — Flutter FFI

**Goal:** expose `bead-core` to Dart.

- `bead-ffi` crate: thin bridge over `pipeline::generate_pattern`. No business
  logic.
- C ABI via `flutter_rust_bridge` (or `cbindgen` + `dart:ffi`); cross-compile
  for iOS (arm64) and Android (arm64-v8a, armeabi-v7a, x86_64).
- Pass image bytes + options + palette in, get pattern + stats + image bytes
  out.

**Done when:** a Dart unit test calls through the bridge and gets the **same
result the CLI produces** for the same input.

---

## M9 — Mobile MVP

**Goal:** a shippable app.

- Flutter app under `apps/mobile`, layered presentation / application / domain
  / infrastructure (per ARCHITECTURE.md).
- Screens: `HomePage` → `CropPage` → `GeneratePage` → `ResultPage`.
- `image_picker`, `crop_your_image`, `riverpod`, `go_router`.
- Copy-summary to clipboard; bundle the default palettes.
- iOS + Android release builds, store metadata, icons, signing.

**Done when:** the success criteria from INIT.md are met (pick image →
generate → preview → counts → copy summary, fully offline) and signed release
builds upload to App Store Connect and Google Play Console.

---

## Notes

- **Determinism is a gate, not a nicety.** Every milestone from M2 on must
  produce identical output for identical input — that is what makes golden
  tests (M7) and the "CLI == FFI" check (M8) possible.
- **The CLI is the contract.** If M9's app ever disagrees with `bead-cli`, the
  bug is in the shell, not the engine.
- Phase 2 algorithm work (color reduction, CIELAB/Delta E, dithering) slots in
  *behind the existing traits* after M9 without reordering this roadmap.
