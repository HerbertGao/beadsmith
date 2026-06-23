# Beadsmith

A local-first pixel bead pattern generator. Convert images into bead-art
patterns, preview results, and generate bead color/count summaries.

The project is designed around a reusable Rust core engine with multiple
frontends (CLI now; Flutter Mobile, Desktop, and Web later).

---

## Vision

Build an open-source, fully local bead pattern generator. Users can:

- Select an image
- Crop the image
- Resize to target bead dimensions
- Convert colors into real bead palette colors
- Preview the final bead pattern
- Generate a color/count summary
- Export pattern assets

No cloud service is required. All computation happens locally.

---

## Design Principles

### Local First

No backend required. No image upload. All processing happens on device.

### Engine First

The Rust engine is the source of truth. All future applications depend on the
same engine. Frontends must not implement their own image processing logic.

### Deterministic

Given the same image, palette, dimensions, and algorithm options, the output
must always be identical.

### Testable

Core algorithms must be independently testable without UI. The CLI should be
sufficient to validate all engine capabilities.

---

## Target Platforms

| Phase | Target | Platforms |
|-------|--------|-----------|
| 1 | Rust CLI | macOS, Linux, Windows |
| 2 | Flutter Mobile | iOS, Android |
| 3 (optional) | Desktop / Web | macOS, Windows, WebAssembly |

---

## Project Structure

```text
beadsmith/
├─ crates/
│  ├─ bead-core/
│  ├─ bead-cli/
│  └─ bead-ffi/
├─ palettes/
├─ samples/
├─ tests/
└─ apps/
   └─ mobile/
```

---

## Crate Responsibilities

### bead-core

All image processing and pattern generation logic. No UI, no filesystem
assumptions, no platform-specific code.

- **Input:** image bytes, palette, options
- **Output:** bead pattern structure

### bead-cli

Command line interface. Used for testing, batch generation, and algorithm
verification.

### bead-ffi

Future Flutter bridge. Exposes stable APIs from `bead-core`. No business logic.

---

## Core Workflow

```text
Input Image
   ↓
Crop
   ↓
Resize
   ↓
Color Quantization
   ↓
Palette Mapping
   ↓
Optional Dithering
   ↓
Pattern Grid
   ↓
Color Statistics
   ↓
Preview Rendering
   ↓
Summary Generation
```

---

## MVP Requirements

### Image Input

- **Formats:** PNG, JPG, JPEG, WEBP
- **Sources:** file path, memory bytes

### Crop

Phase 1: center crop only. Manual crop can be added later by the frontend —
the core engine should not contain UI crop tools.

### Resize

Convert the image into bead grid dimensions. One pixel equals one bead.

Examples: 40×40, 80×100, 100×100, 150×200.

### Palette System

Support external JSON palette definitions:

```json
{
  "brand": "Artkal",
  "colors": [
    { "code": "S01", "name": "Black", "rgb": "#000000" }
  ]
}
```

- **Supported:** Artkal, Perler, Hama
- **Future:** user custom palettes

---

## Pattern Generation

### Phase 1 — Nearest Color Matching

```text
Image Pixel → Find nearest palette color → Replace pixel
```

No color reduction.

### Phase 2 — Color Reduction

Candidate algorithms: Median Cut, K-Means.

Options: `max_colors` (e.g. `max_colors = 24`).

### Phase 3 — Perceptual Color Matching

Use CIELAB + Delta E instead of raw RGB distance.

### Phase 4 — Dithering

Candidate: Floyd–Steinberg. Optional feature.

---

## Data Models

```rust
pub struct PaletteColor {
    pub code: String,
    pub name: String,
    pub rgb: [u8; 3],
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
    pub stats: Vec<ColorStat>,  // filled from M4; M3's BeadPattern has no stats field
}
```

---

## CLI Requirements

```bash
bead-cli generate \
  --input photo.jpg \
  --palette palettes/artkal_s.json \
  --width 80 \
  --height 100 \
  --output ./result
```

Output:

```text
result/
  preview.png
  grid.png
  pattern.json
  summary.txt
```

---

## Summary Format

```text
Bead Pattern Summary
Size: 80 x 100
Total Beads: 8000
Palette: Artkal S

S01 Black: 1240
S02 White: 980
S13 Skin: 760
S45 Brown: 520
```

This text should be directly copyable in mobile applications. CSV export is
not required.

---

## Rendering Requirements

### Preview Image

Rendered without coordinates. Represents the final bead appearance.

### Grid Image

Rendered with row numbers, column numbers, and cell boundaries. Used during
assembly.

---

## Testing Strategy

### Unit Tests

Test palette loading, color matching, and statistics generation.

### Golden Tests

Given a fixed image, fixed palette, and fixed settings, the expected output
must not change. Stored under `tests/golden/`:

```text
preview.png
pattern.json
summary.txt
```

### Benchmark Tests

Use Criterion. Benchmark sizes: 40×40, 80×100, 100×100, 150×150, 300×300.

Track execution time and memory usage.

---

## Non Goals

Not in MVP: user accounts, cloud sync, online generation, AI image generation,
marketplace integration, inventory management, price calculation.

---

## Future Features

- **Inventory Mode** — user owns specific bead colors; the generator
  prioritizes available colors.
- **Multi Palette Mode** — allow replacement between brands (e.g. Artkal →
  Perler alternative).
- **Portrait Enhancement** — optimize for faces and skin tones.
- **Anime Mode** — optimize for line art and high contrast.
- **Board Split Mode** — automatically split large projects into multiple bead
  boards.

---

## Recommended Technology Stack

### Rust Core Engine

`image`, `serde`, `serde_json`, `rayon`, `anyhow`, `thiserror`, `clap`,
`criterion`

### Flutter Frontend

`image_picker`, `crop_your_image`, `riverpod`, `go_router`

---

## Success Criteria

A user can:

1. Select an image
2. Generate a bead pattern
3. View the bead preview
4. View color counts
5. Copy the summary text
6. Complete the workflow without any backend service

The Rust engine should remain reusable across all future platforms.
