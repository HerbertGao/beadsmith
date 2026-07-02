# CLAUDE.md

Guidance for working in this repository.

## What this is

Beadsmith — a local-first pixel bead pattern generator. A reusable Rust engine
(`bead-core`) turns images into bead-art patterns; thin frontends (CLI now,
Flutter mobile later) wrap it. Everything runs on-device; no backend.

Read `INIT.md` (product), `ARCHITECTURE.md` (internals), and `ROADMAP.md`
(milestones M0–M9) before non-trivial work. We are currently at **M9**.

## Layout

```text
crates/bead-core/   engine: all image/palette/pattern logic (library)
crates/bead-cli/    CLI wrapper over bead-core (no algorithms)
crates/bead-ffi/    Flutter bridge — created at M8, not before
```

Other dirs (`palettes/`, `samples/`, `tests/golden/`, `apps/mobile/`) are
created when a milestone first needs them, not up front.

## Hard rules (from ARCHITECTURE.md + ROADMAP.md)

1. **`bead-core` knows nothing about UI, filesystem, Flutter, or platforms.**
   Data in (bytes, options, palette), data out (pattern, stats, images).
2. **Deterministic.** Same input ⇒ identical output. No randomness unless
   explicitly requested. This is a gate, not a nicety — golden tests and the
   "CLI == FFI" check depend on it.
3. **`BeadPattern` is the source of truth.** Preview, statistics, and exports
   all derive from it. Never derive statistics from rendered images.
4. **`pipeline::generate_pattern` is the only generation/orchestration entry**
   for external callers (CLI, FFI): don't re-assemble the
   image→match→stats→render pipeline outside it. Input parsing (`load_palette`)
   and output serialization (`pattern_json`) stay public helpers — the rule
   forbids redoing orchestration externally, not exposing more than one `pub fn`.
   Don't reach into internal pipeline stages from outside.
5. **The CLI is the contract.** If a frontend disagrees with `bead-cli`, the
   bug is in the frontend.

## Build & test

```bash
cargo build
cargo test          # unit + golden tests live in-crate / under tests/golden
cargo run -p bead-cli -- --help
```

## Conventions
- OpenSpec 命令语言按当前项目的 `openspec/config.yaml` 决定：若该配置偏英文（English），使用原版 `openspec`；否则使用 `openspec-cn` 并用中文编写提案/规格。

- Errors: `thiserror` in core, expose `Result<T, BeadError>`. `anyhow` in the
  CLI only.
- Future algorithms slot in behind traits (`BeadReducer`, `ColorMatcher`,
  `Renderer`) without touching the pipeline.
- Performance: single-threaded through Phase 1; `rayon` arrives in Phase 2.
- Spec-driven changes go through OpenSpec (`openspec/`).
