# Beadsmith Architecture

This document describes the internal architecture of Beadsmith. The primary
goal is to keep all image processing logic inside a reusable Rust engine while
allowing multiple frontends to share the same implementation.

---

## High Level Architecture

```text
┌─────────────┐
│ Flutter App │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  bead-ffi   │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  bead-core  │
└──────┬──────┘
       │
   ┌───┼───────────┐
   ▼   ▼           ▼
 Image Proc   Palette   Rendering
```

CLI tools call `bead-core` directly. Mobile applications call `bead-core`
through `bead-ffi`.

---

## Repository Layout

```text
beadsmith/
├─ crates/
│  ├─ bead-core/
│  ├─ bead-cli/
│  └─ bead-ffi/
├─ palettes/
├─ samples/
├─ tests/
├─ docs/
└─ apps/
   └─ mobile/
```

---

## Core Design Rules

### Rule 1 — Isolation

`bead-core` must not know anything about: Flutter, Android, iOS, filesystem,
UI, clipboard, permissions.

### Rule 2 — Data Only

`bead-core` only processes data.

- **Input:** bytes, options, palette
- **Output:** pattern, statistics, rendered images

### Rule 3 — Determinism

All algorithms must be deterministic. No randomness unless explicitly
requested.

Scope (clarified at M7): "identical output for identical input" is
**per-platform / per-architecture / per-`image`-version**. Pure-integer paths
(`RgbMatcher`, statistics, renderer geometry) are bit-identical across
architectures; the floating-point paths — the `Lanczos3` resize (its weights run
`f32::sin`) and the default `LabMatcher` (CIELAB + ΔE76, `cbrt`/`powf`) — are
**not** guaranteed bit-identical across architectures / libm. Golden byte-freezing is
therefore canonical-only on arm64 Linux (CI `ubuntu-24.04-arm`); other platforms
verify float-independent structural invariants. This is exactly what the golden
tests and the future "CLI == FFI" (same-device) check require.

---

## bead-core

The engine crate. Contains all business logic.

### Module Structure

```text
bead-core/
  src/
  ├─ lib.rs
  ├─ image/
  ├─ palette/
  ├─ quantizer/
  ├─ matcher/
  ├─ renderer/
  ├─ statistics/
  ├─ models/
  ├─ pipeline/
  └─ errors/
```

---

### image Module

Responsible for image decoding, resizing, and preprocessing.

```rust
pub fn decode_image(...)
pub fn resize_image(...)
pub fn crop_center(...)
pub fn image_to_grid(...)
```

---

### palette Module

Responsible for palette loading, validation, and indexing.

Input:

```json
{ "brand": "Artkal", "colors": [] }
```

```rust
pub fn load_palette(...)
pub fn validate_palette(...)
```

---

### quantizer Module

Responsible for reducing colors.

- **Phase 1:** not enabled
- **Phase 2:** Median Cut (`MedianCutQuantizer`) — implemented as the default; K-Means (future)

```rust
pub trait Quantizer {
    fn quantize(...)
}
```

---

### matcher Module

Maps image colors to real bead colors. One of the most important modules.

- **Phase 1:** RGB Euclidean distance (`RgbMatcher`)
- **Phase 3:** CIELAB (Lab color space), distance via Delta E (`LabMatcher`) — implemented as the default

```rust
pub trait ColorMatcher {
    fn find_best_match(...)
}
```

---

### renderer Module

Responsible for generating preview images.

- **Preview Renderer** → `preview.png` (no coordinates)
- **Grid Renderer** → `grid.png` (row numbers, column numbers, grid lines)

```rust
pub fn render_preview(...)
pub fn render_grid(...)
```

---

### statistics Module

Responsible for bead counts.

```text
S01 Black: 1240
S02 White: 980
```

```rust
pub fn count_colors(...)
pub fn total_beads(...)
pub fn generate_summary(...)
```

---

### pipeline Module

Orchestrates the entire generation process. This is the main entry point.

```text
Load Image
   ↓
Resize
   ↓
Quantize
   ↓
Match Palette
   ↓
Generate Grid
   ↓
Generate Statistics
   ↓
Render Preview
   ↓
Return Result
```

```rust
pub fn generate_pattern(...)
```

All external callers should use this API.

---

## Data Model Layer

Located in `models/`.

```rust
pub struct PaletteColor {
    pub code: String,
    pub name: String,
    pub rgb: [u8; 3],
}

pub struct PixelGrid {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[u8; 3]>,
}

pub struct ColorStat {
    pub code: String,
    pub name: String,
    pub count: u32,
}

pub struct BeadPattern {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<u16>,         // row-major palette indices; cells[y*width+x]
    // No stats field. Statistics are a derived artifact computed on demand by
    // count_colors(&BeadPattern, &Palette); grid+stats are packaged together in
    // the M6 pipeline layer, not stored on BeadPattern.
}
```

`BeadPattern.cells` is row-major (`cells[y*width+x]` is the palette index of
cell `(x, y)`), with no per-cell coordinates — `(x, y)` is recoverable from the
position, matching `PixelGrid`'s row-major layout. There is no per-cell struct;
cells are bare palette indices.
Statistics are never stored as a field: they are derived on demand by
`count_colors` from `cells` (see M4-D1). `BeadPattern` always holds only
`{width, height, cells}`.

`PixelGrid` is a transitional, raw-RGB intermediate produced by the `image`
module in M2 (row-major, `pixels.len() == width × height`), before any palette
exists. It is **not** the final result: in M3 the matcher consumes a
`PixelGrid` and maps each cell's raw RGB into a `BeadPattern` (resolving the
palette index). `BeadPattern` remains the stable, source-of-truth result for
external callers.

---

## Rendering Strategy

The grid itself is the source of truth. Everything else is derived.

```text
BeadPattern
   ↓
Preview
   ↓
Statistics
   ↓
Exports
```

Before color matching (M2), the raw `PixelGrid` is the source of truth. Once
the matcher resolves cells to palette colors (M3 onward), `BeadPattern` becomes
the source of truth and `PixelGrid` is demoted to a pre-matching intermediate,
no longer returned to external callers. Preview, statistics, and exports all
derive from `BeadPattern`: M4 statistics count over `BeadPattern.cells`
(palette indices), and M5 rendering looks each `cells[i]` up in the palette to
color the cell — never reaching back into `PixelGrid`'s raw RGB.

Never derive statistics from rendered images. Always derive from
`BeadPattern`.

---

## bead-cli

Command line wrapper. Contains no algorithms.

**Responsibilities:** parse arguments, load files, call `bead-core`, save
output.

```bash
bead-cli generate
```

Future commands:

```bash
bead-cli palette list
bead-cli palette validate
bead-cli benchmark
bead-cli inspect
```

---

## bead-ffi

Bridge layer used by Flutter. No business logic.

```text
Dart Objects → Rust Objects → Rust Objects → Dart Objects
```

M8 shipped a thin bridge: it wraps a single `generate_pattern` call whose
boundary is `width`/`height` only, `bead-core` stays zero-change, and the host
dynamic library is proven against the CLI by a same-device Dart determinism
test. The filter/cell_size/shape option knobs remain engine defaults.

M9 added mobile packaging with **no bridge-logic change** (the crate gains a
`staticlib` crate-type alongside `cdylib`). **iOS cross-compilation is done and
verified**: `scripts/build-ios.sh` produces `libbead_ffi.a` for device
(`aarch64-apple-ios`) and simulator (`aarch64-apple-ios-sim`,
`x86_64-apple-ios`), linked into the Flutter Runner and loaded via FRB's
`ExternalLibrary.process()`. **Android scaffold is in place but unverified**:
jniLibs directory layout + a Gradle/NDK build hook + `ExternalLibrary.open`
loader branch exist, but actual cross-compilation and on-device validation are
deferred to a user environment with the Android SDK/NDK installed
(see `apps/mobile/android/RUST_BUILD_TODO.md`).

---

## Flutter Architecture

Future implementation. Four layers:

- **presentation** — screens: `HomePage`, `CropPage`, `GeneratePage`,
  `ResultPage`
- **application** — use cases: `GeneratePattern`, `CopySummary`, `SaveProject`
- **domain** — shared entities: `Project`, `Palette`, `Pattern`
- **infrastructure** — bridge to Rust: `PatternEngine`, `ClipboardService`

---

## Error Handling

Use `thiserror` inside core. Expose `Result<T, BeadError>` everywhere.

---

## Performance Strategy

- **Phase 1:** single-threaded
- **Phase 2:** use `rayon` to parallelize pixel matching, statistics, and
  rendering

---

## Testing Strategy

- **Unit Tests** — per module (palette, matcher, statistics)
- **Golden Tests** — verify output stability; store under `tests/golden/`
  (arm64-Linux-canonical byte freeze + cross-platform structural invariants, Rule 3)
- **Benchmark Tests** — use Criterion; track runtime (memory / throughput
  deferred to Phase-2 — see INIT.md → Benchmark Tests)

---

## Future Plugin Architecture

Future algorithms should be swappable via traits:

```rust
pub trait Quantizer
pub trait ColorMatcher
pub trait Renderer
```

This allows RGB Matcher, Lab Matcher, or a Custom Matcher without changing the
pipeline.

---

## Architectural Goal

The Rust engine should eventually become a standalone reusable library.

Potential consumers: Flutter Mobile, Desktop Application, WebAssembly, CLI
Tools, third-party integrations.

All platforms should produce identical results when given identical inputs.
