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
- **Phase 2:** Median Cut, K-Means

```rust
pub trait Quantizer {
    fn quantize(...)
}
```

---

### matcher Module

Maps image colors to real bead colors. One of the most important modules.

- **Phase 1:** RGB Euclidean distance
- **Phase 2:** CIELAB (Lab color space), distance via Delta E

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
S01 Black 1240
S02 White 980
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

pub struct BeadCell {
    pub x: u32,
    pub y: u32,
    pub color_index: u16,
}

pub struct ColorStat {
    pub code: String,
    pub name: String,
    pub count: u32,
}

pub struct BeadPattern {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<BeadCell>,
    pub stats: Vec<ColorStat>,
}
```

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
- **Benchmark Tests** — use Criterion; track runtime, memory, throughput

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
