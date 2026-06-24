//! Criterion time baseline for `generate_pattern` end-to-end (M7, design D8).
//!
//! Benches the library entry point `generate_pattern` — what the CLI and the
//! future FFI layer actually call — over five fixed *target* sizes. Inputs are
//! synthesized in-bench (same formula as M6's `demo_png`), never committed as
//! large fixtures. The synthesized source image is **2× the target on each
//! axis**: a source equal to the target hits `imageops::resize`'s "src == dst"
//! copy short-circuit, which would skip the Lanczos resample and make the
//! benchmark miss the resize cost entirely.
//!
//! ponytail: 测库入口 generate_pattern 端到端（M8/FFI 与 CLI 真调的东西）；
//! per-stage 拆分留 Phase-2 优化基线、非必需。

use std::hint::black_box;

use bead_core::{generate_pattern, load_palette, GenerateOptions, Palette};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use image::{Rgb, RgbImage};

/// Five fixed *target* grid sizes (cells), as in design D8.
const TARGET_SIZES: [(u32, u32); 5] = [(40, 40), (80, 100), (100, 100), (150, 150), (300, 300)];

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
        // Source = 2× target on each axis, so the Lanczos resample actually runs
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

criterion_group!(benches, bench_generate_pattern);
criterion_main!(benches);
