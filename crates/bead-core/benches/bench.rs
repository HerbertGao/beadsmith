//! Criterion time baseline for `generate_pattern` end-to-end (M7, design D8).
//!
//! Benches the library entry point `generate_pattern` — what the CLI and the
//! future FFI layer actually call — over five fixed *target* sizes. Inputs are
//! synthesized in-bench (same formula as M6's `demo_png`), never committed as
//! large fixtures. The synthesized source image is **2× the target on each
//! axis**: a source equal to the target hits `imageops::resize`'s "src == dst"
//! copy short-circuit, which would skip the Triangle resample and make the
//! benchmark miss the resize cost entirely.
//!
//! ponytail: 测库入口 generate_pattern 端到端（M8/FFI 与 CLI 真调的东西）；
//! per-stage 拆分留 Phase-2 优化基线、非必需。

use std::hint::black_box;

use bead_core::{generate_pattern, load_palette, GenerateOptions, GeneratorKind, Palette};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use image::{Rgb, RgbImage};

/// Five fixed *target* grid sizes (cells), as in design D8.
const TARGET_SIZES: [(u32, u32); 5] = [(40, 40), (80, 100), (100, 100), (150, 150), (300, 300)];

/// Gerstner *target* grid sizes. Smaller than the Staged set on purpose: the
/// Gerstner v1 front end is `O(source pixels × T)` (fixed `T` iterations over the
/// *whole source*, not the grid), so cost is driven by the SOURCE size below, not
/// the target.
const GERSTNER_TARGET_SIZES: [(u32, u32); 3] = [(32, 32), (48, 60), (64, 64)];

/// Source = this multiple of the target on each axis for the Gerstner bench.
/// Gerstner requires `target ≤ source` (upsampling guard), and a source strictly
/// larger than the target is what makes the superpixel windows actually
/// downsample — with a `k×` source the SLIC pass iterates over `k²×` more pixels,
/// which is where the `O(source × T)` cost lives.
const GERSTNER_SOURCE_MULT: u32 = 3;

/// Synthesize a source PNG, encoded to in-memory bytes (M6 `demo_png` formula).
/// `w`/`h` are the *source* dimensions (caller passes 2× the target).
fn demo_png(w: u32, h: u32) -> Vec<u8> {
    let img = RgbImage::from_fn(w, h, |x, y| {
        Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
    });
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut buf, image::ImageFormat::Png)
        .expect("encoding the bench PNG must succeed");
    buf.into_inner()
}

/// Load `palettes/artkal_s.json`, embedded at compile time via `include_bytes!`
/// so `bead-core` (incl. its bench harness) touches **no filesystem at runtime**
/// (CLAUDE.md rule 1 / design D2: core is data-in/data-out, fs-free).
fn load_artkal_s() -> Palette {
    let bytes = include_bytes!("../../../palettes/artkal_s.json");
    load_palette(bytes).expect("artkal_s palette must parse")
}

fn bench_generate_pattern(c: &mut Criterion) {
    let palette = load_artkal_s();
    let mut group = c.benchmark_group("generate_pattern");

    for (w, h) in TARGET_SIZES {
        // Source = 2× target on each axis, so the Triangle resample actually runs
        // (a src == dst source would hit the resize copy short-circuit).
        let png_bytes = demo_png(2 * w, 2 * h);
        let opts = GenerateOptions {
            width: w,
            height: h,
            ..Default::default()
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{w}x{h}")),
            &(png_bytes, opts),
            |b, (png_bytes, opts)| {
                b.iter(|| {
                    generate_pattern(black_box(png_bytes), black_box(&palette), black_box(opts))
                        .expect("generate_pattern must succeed")
                });
            },
        );
    }

    group.finish();
}

/// Gerstner-path baseline (tasks §8.1). Same entry point (`generate_pattern`) with
/// `generator = Gerstner`, over a few target sizes with a `k×` source so the SLIC
/// superpixel pass actually runs.
///
/// **v1 performance characteristics (honest):** the Gerstner front end is
/// single-threaded and `O(source pixels × T)` — `T` fixed iterations, each a full
/// assign + centroid-update pass over *every source pixel*. So cost scales with
/// the SOURCE image, not the bead grid: a large photo is slow (e.g. a 12 MP source
/// × T=10 ≈ 1e9 candidate evals). The v1 mitigation is to **pre-shrink the input**
/// before calling; source-pixel clamping and `rayon` parallelism are deferred to
/// Phase 2 (rayon must preserve the fixed row-major centroid accumulation order to
/// keep determinism). `criterion` stays a dev-dependency only.
fn bench_generate_pattern_gerstner(c: &mut Criterion) {
    let palette = load_artkal_s();
    let mut group = c.benchmark_group("generate_pattern_gerstner");

    for (w, h) in GERSTNER_TARGET_SIZES {
        // Source = k× target on each axis (target ≤ source; superpixels downsample).
        let png_bytes = demo_png(GERSTNER_SOURCE_MULT * w, GERSTNER_SOURCE_MULT * h);
        let opts = GenerateOptions {
            width: w,
            height: h,
            generator: GeneratorKind::Gerstner,
            ..Default::default()
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{w}x{h}")),
            &(png_bytes, opts),
            |b, (png_bytes, opts)| {
                b.iter(|| {
                    generate_pattern(black_box(png_bytes), black_box(&palette), black_box(opts))
                        .expect("Gerstner generate_pattern must succeed")
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_generate_pattern,
    bench_generate_pattern_gerstner
);
criterion_main!(benches);
