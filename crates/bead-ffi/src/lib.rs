//! `bead-ffi` — the thin `bead-core` → Dart bridge (M8). Zero business logic:
//! it only calls the existing public engine APIs (`load_palette`,
//! `generate_pattern`, `pattern_json`) and marshals the result across
//! `flutter_rust_bridge` (FRB). `bead-core` stays untouched — the cross-boundary
//! DTOs of `BeadPattern` / `ColorStat` live here (CLAUDE rule 1 / ARCHITECTURE
//! "bead-ffi" rule). M8 is host-only; the boundary is `width` / `height` only.
//!
//! See `openspec/changes/add-flutter-ffi/` for the full contract.

pub mod api;

mod frb_generated;
