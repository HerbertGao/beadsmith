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

- `image` module: `decode_image`, `crop_center`, `resize_image`, `image_to_grid`
  (PNG/JPG/JPEG/WEBP in).
- `models`: `PixelGrid` (raw row-major RGB grid; `BeadPattern { cells: Vec<u16> }`
  comes in M3 once there is a palette to index into).
- Center-crop to target aspect ratio, then resize to `width × height`.

**Done when:** decoding + center-crop + resize to e.g. 80×100 produces a
`PixelGrid` with exactly `width × height` cells; a golden-ish fixture
confirms deterministic output.

---

## M3 — Color Matching

**Goal:** map each cell to the nearest real bead color (Phase 1 = RGB).

- `matcher` module: `ColorMatcher` trait + `find_best_match`.
- Phase 1 implementation: RGB Euclidean distance against the loaded palette.
- Introduce `BeadPattern { cells: Vec<u16> }` (row-major palette indices) and
  map M2's `PixelGrid` into a `BeadPattern` (each cell's raw RGB resolved to a
  palette index).
- Each `cells[i]` (a `u16`) now points into the palette.

**Done when:** known colors map to their exact palette entry; an off-palette
color maps to the nearest one (asserted in a unit test). Deterministic.

---

## M4 — Statistics

**Goal:** count beads per color from the pattern.

- `statistics` module: `count_colors`, `total_beads`, `generate_summary`.
- `ColorStat` model; statistics derived from `BeadPattern`
  (count_colors/total_beads/generate_summary).
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

- Flutter app under `apps/mobile`, layered as the MVP three layers
  presentation / application / infrastructure (domain deferred until
  persistence / `SaveProject` lands — see ARCHITECTURE.md).
- Screens: `HomePage` → `CropPage` → `GeneratePage` → `ResultPage`.
- `image_picker`, `crop_your_image`, `riverpod`, `go_router`.
- Copy-summary to clipboard; bundle the default palettes.
- Bundle the default palettes; copy-summary to clipboard.

**Done when:** the success criteria from INIT.md are met — pick image →
generate → preview → counts → copy summary, fully offline — verified on iOS (see
**Status**). Signed release builds, store metadata/icons, and upload to App Store
Connect / Google Play are **deferred to a separate later change** (release
engineering, not part of this milestone's gate).

> **Status:** the offline app is achieved this milestone — iOS is cross-compiled
> and the four-screen flow runs on-device, fully offline. Android was shipped as
> an unverified scaffold at M9 and **verified post-M9** on an Android emulator
> (Pixel_10, Android 17 / API 37): the `libbead_ffi.so` per-ABI build +
> jniLibs packaging + `ExternalLibrary.open` load + the same four-screen flow
> all run, automated by `integration_test/engine_on_android_test.dart` plus a
> one-time manual run. See `apps/mobile/android/RUST_BUILD.md`. Store signing,
> metadata, and upload to App Store Connect / Google Play are **deferred to a
> separate later change** (release engineering, not MVP function).

---

## Post-M9 — Engine Algorithm Upgrades

Phase 2/3 algorithm work, slotted in *behind the existing traits*
(`ColorMatcher` / `BeadReducer`) and the `pipeline` generation seam **without
reordering M0–M9**. All spec-driven via OpenSpec, deterministic, CLI-validated;
the default `Staged` output and its golden are unchanged unless noted.

- **Oklab matcher** ✅ — `OklabMatcher` (Oklab + ΔEok²) under `ColorMatcher`;
  `--matcher rgb|lab|oklab`, default `oklab`.
- **Palette-aware reduction** ✅ — `GreedyReducer` (`BeadReducer` trait) merges to
  `≤N` bead colors *after* matching (`--max-colors`), replacing the earlier
  pre-match quantizer; default resize filter moved to `Triangle`.
- **Despeckle** ✅ — connected-component cleanup of isolated specks
  (`--despeckle <min-region>`); pure integer, cross-arch bit-exact.
- **Gerstner generation mode** ✅ — opt-in deterministic SLIC-variant superpixel
  front end (`--generator staged|gerstner`) for photo/portrait drafts; default
  stays `Staged`.
- **Dithering** — deferred (algorithm Phase 4, off by default; negative for
  solid beads).

> Canonical byte golden is enforced on arm64 Linux CI; the f32 paths (`Triangle`,
> `Oklab`, `Gerstner`) are **same-machine canonical**, not cross-arch bit-exact.

---

## Post-M9 — Mobile UI Refinement

Frontend-only polish of the M9 four-screen flow, plus surfacing the engine knobs
that already exist but the app can't reach. **No new engine algorithms;
determinism unaffected** — UI work + option pass-through only. Spec-driven via
OpenSpec, and splittable into the three workstreams below, each shippable on its
own (the settings controls depend on the FFI widening landing first).

Design direction — *"pegboard workshop"*: neutral violet-grey chrome so the bead
colors carry the page, rounded bead-like controls, bead codes/counts set in a
mono face. Tokens: accent `#6C4BF4`, secondary `#12A594`, ink `#1C1830`, ground
`#F4F3F7`, line `#E6E3EF`. Pitch mockup:
<https://claude.ai/code/artifact/e80e77a4-c7f0-461e-864c-75fa41c4c144>.

- **Cropper upgrade** — replace `crop_your_image` with a **self-drawn cropper**
  (`CropFrame` widget + pure `crop_geometry` + the `image` package): a fixed
  aspect-ratio viewfinder over a pan/zoom (cover-min) image, aspect presets
  (square / 2:3 / 3:4 / 4:5 / 9:16 with a portrait↔landscape swap), rotate, and
  flip, themed to the tokens. On confirm the shell orients the decoded bytes
  (`copyRotate` then `flipHorizontal`) and `copyCrop`s the framed rect in
  oriented-image space — **no `RepaintBoundary.toImage`** (it fails on the iOS
  simulator's software renderer, which killed the `pro_image_editor` route).
  Keeps the `Uint8List` cropped-bytes hand-off (no file-path rewrite), and feeds
  the crop aspect to the generate screen to lock the bead-grid ratio.
- **Widen the FFI boundary** — pass `max_colors` / `despeckle` / `generator`
  (`staged|gerstner`) through `generate`; **supersedes the deliberate M8
  "width/height only" boundary**. Engine side is already done (see the engine
  Post-M9 above); the Settings-screen controls are dead until this lands.
- **Four-screen restyle** — Home / Crop / Settings / Result rebuilt on the tokens
  above, plus dark mode. Result still derives its stats/legend **verbatim from
  `GenerateOutput`**, never from the rendered image (hard rule).

> Store release engineering (signing, icons, store metadata, upload) stays
> deferred (see M9 **Status**) — separate from this UI pass.

---

## Notes

- **Determinism is a gate, not a nicety.** Every milestone from M2 on must
  produce identical output for identical input — that is what makes golden
  tests (M7) and the "CLI == FFI" check (M8) possible.
- **The CLI is the contract.** If M9's app ever disagrees with `bead-cli`, the
  bug is in the shell, not the engine.
- Phase 2/3 algorithm work (color reduction, Oklab/ΔE, despeckle, Gerstner) has
  landed *behind the existing traits* after M9 without reordering this roadmap
  (see **Post-M9**); dithering remains deferred (algorithm Phase 4).
