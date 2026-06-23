//! Image decoding, normalization, center-cropping, and resizing into a fixed
//! `PixelGrid`. Pure data in (bytes, options), pure data out (grid) — no
//! filesystem, no UI, no platform assumptions (see ARCHITECTURE.md).
//!
//! The local module is named `image`, which collides with the external `image`
//! crate. Every reference to the external crate therefore uses a leading `::`
//! (`::image::…`) or the `use ::image::{…}` imports below; a bare `image::…`
//! path would resolve to this local module and fail to compile (see design D6).
//!
// ponytail: EXIF orientation is read-but-not-applied. `image 0.25`'s
// `load_from_memory` / `DynamicImage::from_decoder` decode at the *stored*
// orientation and do not call `apply_orientation`, so a JPEG with an EXIF
// orientation tag decodes unrotated. M2 depends on this default and does not
// introduce auto-rotation (see design D9). If this ever changes to
// auto-rotate, the `exif_orientation_not_applied` test (group 3) must fail
// loudly.

use ::image::imageops::FilterType;
use ::image::RgbImage;

use crate::models::PixelGrid;
use crate::BeadError;

/// Options controlling how an image is resized into the target grid.
///
/// `filter` is the external `image` crate's [`FilterType`]; it leaks into this
/// public signature deliberately, in exchange for primitive reuse (see design
/// D3). The default is `Lanczos3` (design D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResizeOptions {
    /// Resampling filter used when scaling to the target grid.
    pub filter: FilterType,
}

impl Default for ResizeOptions {
    fn default() -> Self {
        ResizeOptions {
            filter: FilterType::Lanczos3,
        }
    }
}

/// Decodes **and normalizes to RGB8** (drops alpha, downsamples 16-bit). Auto-
/// sniffs the format from the bytes; PNG / JPEG / WEBP are compiled in.
///
/// All color types are funneled through `to_rgb8()`: alpha is dropped (not
/// flattened), Luma8 maps to `r == g == b`, 16-bit channels are downsampled to
/// 8-bit, and palette PNGs are expanded to RGB (design D4). Corrupt, truncated,
/// or not-compiled-in formats return [`BeadError::ImageDecode`] via `#[from]`.
pub fn decode_image(bytes: &[u8]) -> Result<RgbImage, BeadError> {
    Ok(::image::load_from_memory(bytes)?.to_rgb8())
}

/// Center-crops `src` to the largest sub-rectangle whose aspect ratio matches
/// the target `tw : th`. Crop offsets floor with the integer division; any odd
/// leftover pixel goes to the right / bottom. When the source ratio is
/// **exactly** equal to the target ratio, the crop is a no-op (the whole image
/// is returned unchanged).
///
/// This is a public primitive (design D3) and therefore self-guards its own
/// degenerate inputs, returning [`BeadError::InvalidImage`] (with a
/// deterministic `reason` naming the offending dimension) rather than relying
/// on any orchestration order (design D5′). It never produces a zero-dimension
/// intermediate image and never panics. Rejected inputs:
/// - target `tw == 0` or `th == 0` (degenerate target ratio);
/// - source with a zero dimension;
/// - an extreme aspect ratio whose computed crop width / height floors to 0.
///
/// Ratio comparison and crop-size derivation use integer cross-multiplication
/// (`src_w * th` vs `src_h * tw`), widened to `u64` first so that small-but-
/// extreme inputs (e.g. `100000×1` against `1×100000`, whose `u32` product
/// `1e10` would overflow) neither panic in debug nor wrap in release before the
/// dimension is validated and `checked_cast` back to `u32`.
pub fn crop_center(src: &RgbImage, tw: u32, th: u32) -> Result<RgbImage, BeadError> {
    // Guard 1: degenerate target ratio. A zero target dimension would force the
    // crop width / height to floor to 0.
    if tw == 0 {
        return Err(BeadError::InvalidImage {
            reason: "crop target width is 0".to_string(),
        });
    }
    if th == 0 {
        return Err(BeadError::InvalidImage {
            reason: "crop target height is 0".to_string(),
        });
    }

    let src_w = src.width();
    let src_h = src.height();

    // Guard 2a: zero-dimension source.
    if src_w == 0 {
        return Err(BeadError::InvalidImage {
            reason: "source width is 0".to_string(),
        });
    }
    if src_h == 0 {
        return Err(BeadError::InvalidImage {
            reason: "source height is 0".to_string(),
        });
    }

    // All products widen to u64 first: two u32 multiplied can reach ~1.84e19,
    // which fits in u64. This avoids the u32 overflow that a small-but-extreme
    // input (e.g. 100000×1 vs 1×100000, product 1e10) would otherwise hit.
    let src_w64 = src_w as u64;
    let src_h64 = src_h as u64;
    let tw64 = tw as u64;
    let th64 = th as u64;

    // Cross-multiply to compare src_w/src_h against tw/th without floats or
    // dividing by (tw/th).
    let src_ratio = src_w64 * th64; // src_w / src_h ? tw / th  →  src_w*th ? src_h*tw
    let tgt_ratio = src_h64 * tw64;

    // Exact-ratio match: no-op (return the whole image unchanged).
    if src_ratio == tgt_ratio {
        return Ok(src.clone());
    }

    // Compute the largest centered sub-rectangle matching tw:th.
    //
    // If the source is wider than the target ratio (src_w*th > src_h*tw), the
    // height is the limit: crop_h = src_h, crop_w = src_h * tw / th.
    // Otherwise the width is the limit: crop_w = src_w,
    // crop_h = src_w * th / tw.
    let (crop_w64, crop_h64) = if src_ratio > tgt_ratio {
        // Wider than target → constrain by height.
        (src_h64 * tw64 / th64, src_h64)
    } else {
        // Taller than target → constrain by width.
        (src_w64, src_w64 * th64 / tw64)
    };

    // Guard 2b: the derived crop floored to a zero dimension.
    if crop_w64 == 0 {
        return Err(BeadError::InvalidImage {
            reason: "computed crop width floored to 0".to_string(),
        });
    }
    if crop_h64 == 0 {
        return Err(BeadError::InvalidImage {
            reason: "computed crop height floored to 0".to_string(),
        });
    }

    // crop_w64 <= src_w and crop_h64 <= src_h, both u32-range, so these casts
    // never truncate; validate-then-cast keeps that guarantee explicit.
    let crop_w = crop_w64 as u32;
    let crop_h = crop_h64 as u32;

    // Center offsets: floor of the difference; odd leftover goes right/bottom.
    let x = (src_w - crop_w) / 2;
    let y = (src_h - crop_h) / 2;

    Ok(::image::imageops::crop_imm(src, x, y, crop_w, crop_h).to_image())
}

/// Resizes `src` to an exact `width × height` `PixelGrid` (aspect ratio is
/// **not** preserved — callers crop first). Upscaling (target larger than
/// source) is allowed. Uses [`::image::imageops::resize`] with
/// `options.filter` (default `Lanczos3`).
///
/// Note: `DynamicImage::resize_exact` is *not* available on `RgbImage`; only
/// the `imageops::resize` free function is (see design D6′).
///
/// This is a public primitive (design D3) and self-guards (design D5′): the
/// target `width == 0 || height == 0`, **or** the source having a zero
/// dimension (either condition rejects), returns [`BeadError::InvalidImage`]
/// before `imageops::resize` is called. (A zero-dimension source silently
/// returns `Ok` + an all-black grid from `imageops::resize`, which must be
/// blocked here.)
pub fn resize_image(
    src: &RgbImage,
    width: u32,
    height: u32,
    options: &ResizeOptions,
) -> Result<PixelGrid, BeadError> {
    if width == 0 {
        return Err(BeadError::InvalidImage {
            reason: "resize target width is 0".to_string(),
        });
    }
    if height == 0 {
        return Err(BeadError::InvalidImage {
            reason: "resize target height is 0".to_string(),
        });
    }
    if src.width() == 0 {
        return Err(BeadError::InvalidImage {
            reason: "source width is 0".to_string(),
        });
    }
    if src.height() == 0 {
        return Err(BeadError::InvalidImage {
            reason: "source height is 0".to_string(),
        });
    }

    let resized = ::image::imageops::resize(src, width, height, options.filter);

    // Row-major raw RGB. `collect` sizes from the resized buffer's exact length
    // (usize), so there is no u32 `width * height` multiply to overflow
    // (see design D1 / PixelGrid invariant). `Rgb<u8>.0` is already `[u8; 3]`.
    let pixels = resized.pixels().map(|p| p.0).collect();

    Ok(PixelGrid {
        width,
        height,
        pixels,
    })
}

/// Decodes image bytes and produces a `width × height` `PixelGrid`: decode +
/// normalize → center-crop to the target ratio → resize to exact dimensions.
///
/// The target `width / height` are validated **first** (before decoding and
/// cropping): `crop_center` derives its ratio from the target, so a zero target
/// would otherwise divide by zero / be rejected only later. A zero target
/// dimension returns [`BeadError::InvalidImage`] (design D5′ guard order).
pub fn image_to_grid(
    bytes: &[u8],
    width: u32,
    height: u32,
    options: &ResizeOptions,
) -> Result<PixelGrid, BeadError> {
    // Validate target dimensions before anything that derives a ratio from
    // them (crop_center divides by the target ratio).
    if width == 0 {
        return Err(BeadError::InvalidImage {
            reason: "target width is 0".to_string(),
        });
    }
    if height == 0 {
        return Err(BeadError::InvalidImage {
            reason: "target height is 0".to_string(),
        });
    }

    let decoded = decode_image(bytes)?;
    let cropped = crop_center(&decoded, width, height)?;
    resize_image(&cropped, width, height, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::image::{Rgb, RgbImage};

    // Deterministic fixtures (bytes committed; generator is throwaway, not in
    // repo). See group-3 spec / design D8–D9.
    //
    // gradient_rgb8.png — 160×200 RGB8 gradient, golden source. Pixel formula
    //   r = x % 256, g = y % 256, b = (x + y) % 256. 160:200 == 4:5, so a 4:5
    //   target crop is a no-op (whole image resized).
    const GRADIENT_PNG: &[u8] = include_bytes!("fixtures/gradient_rgb8.png");
    const TRANSPARENT_RGBA_PNG: &[u8] = include_bytes!("fixtures/transparent_rgba.png");
    const LUMA8_PNG: &[u8] = include_bytes!("fixtures/luma8.png");
    const RGB16_PNG: &[u8] = include_bytes!("fixtures/rgb16.png");
    const PALETTE_PNG: &[u8] = include_bytes!("fixtures/palette.png");
    const SMOKE_JPG: &[u8] = include_bytes!("fixtures/smoke.jpg");
    const SMOKE_WEBP: &[u8] = include_bytes!("fixtures/smoke.webp");
    const EXIF_JPG: &[u8] = include_bytes!("fixtures/exif_orientation.jpg");
    // Recognized-but-not-compiled-in formats (image built with only png/jpeg/
    // webp): decoding these proves the feature set is what we claim.
    const SAMPLE_GIF: &[u8] = include_bytes!("fixtures/sample.gif");
    const SAMPLE_BMP: &[u8] = include_bytes!("fixtures/sample.bmp");
    const SAMPLE_TIFF: &[u8] = include_bytes!("fixtures/sample.tiff");

    fn nearest() -> ResizeOptions {
        ResizeOptions {
            filter: FilterType::Nearest,
        }
    }

    // ---- 6.1 Done-when: exact cell count ------------------------------------

    #[test]
    fn grid_has_exact_cell_count() {
        let grid = image_to_grid(GRADIENT_PNG, 80, 100, &ResizeOptions::default()).unwrap();
        assert_eq!(grid.width, 80);
        assert_eq!(grid.height, 100);
        assert_eq!(grid.pixels.len(), 8000);
    }

    // ---- 6.2 determinism (the real gate) + Nearest bit-exact golden ---------
    //
    // ponytail: 完整 golden-file 框架推迟 M7；确定性靠重算比对 + Nearest 位精确
    // golden，跨架构稳定。

    #[test]
    fn grid_is_deterministic() {
        let opts = ResizeOptions::default();
        let a = image_to_grid(GRADIENT_PNG, 80, 100, &opts).unwrap();
        let b = image_to_grid(GRADIENT_PNG, 80, 100, &opts).unwrap();
        // Full PixelGrid equality (dimensions + every pixel, in order): the true
        // determinism gate, recomputed in-process so it is cross-arch stable.
        assert_eq!(a, b);

        // Inline golden uses Nearest (integer-exact, no f32) so the hardcoded
        // expectation is bit-exact across architectures — NOT Lanczos3's f32
        // output. 160×200 → 4×5 is a 4:5-to-4:5 (no-op crop) Nearest downscale.
        let grid = image_to_grid(GRADIENT_PNG, 4, 5, &nearest()).unwrap();
        assert_eq!(grid.width, 4);
        assert_eq!(grid.height, 5);
        let expected: Vec<[u8; 3]> = vec![
            [20, 20, 40],
            [60, 20, 80],
            [100, 20, 120],
            [140, 20, 160],
            [20, 60, 80],
            [60, 60, 120],
            [100, 60, 160],
            [140, 60, 200],
            [20, 100, 120],
            [60, 100, 160],
            [100, 100, 200],
            [140, 100, 240],
            [20, 140, 160],
            [60, 140, 200],
            [100, 140, 240],
            [140, 140, 24],
            [20, 180, 200],
            [60, 180, 240],
            [100, 180, 24],
            [140, 180, 64],
        ];
        assert_eq!(grid.pixels, expected);
    }

    // ---- 6.3 format coverage: decode png/jpeg/webp --------------------------

    #[test]
    fn decode_png() {
        let img = decode_image(GRADIENT_PNG).unwrap();
        assert_eq!((img.width(), img.height()), (160, 200));
    }

    #[test]
    fn decode_jpeg() {
        let img = decode_image(SMOKE_JPG).unwrap();
        assert_eq!((img.width(), img.height()), (16, 16));
    }

    #[test]
    fn decode_webp() {
        let img = decode_image(SMOKE_WEBP).unwrap();
        assert_eq!((img.width(), img.height()), (16, 16));
    }

    // ---- 6.4 reject garbage and recognized-but-uncompiled formats -----------
    // Assert the variant only, never the underlying `image` Display text.

    #[test]
    fn decode_rejects_garbage() {
        let err = decode_image(b"not an image").unwrap_err();
        assert!(matches!(err, BeadError::ImageDecode(_)));
    }

    #[test]
    fn decode_rejects_unsupported_format() {
        for bytes in [SAMPLE_GIF, SAMPLE_BMP, SAMPLE_TIFF] {
            let err = decode_image(bytes).unwrap_err();
            assert!(
                matches!(err, BeadError::ImageDecode(_)),
                "uncompiled format must be ImageDecode"
            );
        }
    }

    // ---- 6.5 normalization to RGB8 ------------------------------------------

    #[test]
    fn alpha_is_dropped_deterministically() {
        // (0,0) is RGBA(10,20,30,0): fully transparent. Alpha is dropped (not
        // flattened to a background), so the RGB channels survive verbatim.
        let a = decode_image(TRANSPARENT_RGBA_PNG).unwrap();
        let b = decode_image(TRANSPARENT_RGBA_PNG).unwrap();
        assert_eq!(a.get_pixel(0, 0).0, [10, 20, 30]);
        assert_eq!(a.get_pixel(3, 3).0, [200, 100, 50]);
        assert_eq!(a, b, "normalization is deterministic across runs");
    }

    #[test]
    fn luma8_normalized_to_rgb() {
        let img = decode_image(LUMA8_PNG).unwrap();
        for px in img.pixels() {
            let [r, g, b] = px.0;
            assert!(r == g && g == b, "luma maps to r == g == b, got {:?}", px.0);
        }
        assert_eq!(img.get_pixel(0, 0).0, [123, 123, 123]);
    }

    #[test]
    fn rgb16_normalized_to_rgb8() {
        // Must not panic; 16-bit channels downsample to 8-bit (high byte).
        let img = decode_image(RGB16_PNG).unwrap();
        assert_eq!((img.width(), img.height()), (2, 2));
        // 0xFFFF→255, 0x0000→0, 0x8000→128, 0x0100→1.
        assert_eq!(img.get_pixel(0, 0).0, [255, 0, 128]);
        assert_eq!(img.get_pixel(1, 1).0, [1, 1, 1]);
    }

    #[test]
    fn palette_png_expanded() {
        // Indexed PNG: palette idx0..3 = red/green/blue/yellow, expanded to RGB.
        let img = decode_image(PALETTE_PNG).unwrap();
        assert_eq!(img.get_pixel(0, 0).0, [255, 0, 0]);
        assert_eq!(img.get_pixel(1, 0).0, [0, 255, 0]);
        assert_eq!(img.get_pixel(0, 1).0, [0, 0, 255]);
        assert_eq!(img.get_pixel(1, 1).0, [255, 255, 0]);
    }

    // ---- 6.6 center crop ----------------------------------------------------

    #[test]
    fn crop_center_odd_split_biases_right_bottom() {
        // Source 5×1: cropping to a 1:1 target yields crop_w == crop_h == 1.
        // Difference 5-1 == 4 in width → offset floor(4/2) == 2; the kept column
        // is x == 2. Put a sentinel there to prove the offset (and that any odd
        // leftover would bias right/bottom).
        let mut src = RgbImage::new(5, 1);
        src.put_pixel(2, 0, Rgb([9, 9, 9]));
        let out = crop_center(&src, 1, 1).unwrap();
        assert_eq!((out.width(), out.height()), (1, 1));
        assert_eq!(out.get_pixel(0, 0).0, [9, 9, 9], "kept the centered column");

        // Source 4×1 to 1:1 → crop 1×1, diff 3, offset floor(3/2) == 1: the odd
        // leftover pixel falls to the right (x == 1 kept, not x == 2 → would be
        // ceil). Sentinel at x == 1 proves floor + right bias.
        let mut src2 = RgbImage::new(4, 1);
        src2.put_pixel(1, 0, Rgb([7, 7, 7]));
        let out2 = crop_center(&src2, 1, 1).unwrap();
        assert_eq!(out2.get_pixel(0, 0).0, [7, 7, 7]);
    }

    #[test]
    fn crop_center_landscape_to_square() {
        let src = RgbImage::new(20, 10);
        let out = crop_center(&src, 1, 1).unwrap();
        assert_eq!(out.width(), out.height(), "square crop: width == height");
        assert_eq!((out.width(), out.height()), (10, 10));
    }

    #[test]
    fn crop_center_already_matching_ratio_is_noop() {
        // 160×200 == 4:5; target 4:5 → ratio is exactly equal → whole image
        // returned unchanged.
        let src = decode_image(GRADIENT_PNG).unwrap();
        let out = crop_center(&src, 4, 5).unwrap();
        assert_eq!((out.width(), out.height()), (160, 200));
        assert_eq!(out, src, "exact-ratio crop is a no-op");
    }

    #[test]
    fn crop_center_near_equal_ratio_is_not_noop() {
        // 100×99 is *almost* square but not exactly 1:1. The no-op is for EXACT
        // ratio equality only (spec: 仅精确相等才 no-op); a near-equal ratio must
        // still crop. src_ratio = 100*1 = 100 > tgt_ratio = 99*1 = 99 → crop by
        // height → 99×99 (a real crop, not the original 100×99).
        let src = RgbImage::new(100, 99);
        let out = crop_center(&src, 1, 1).unwrap();
        assert_eq!((out.width(), out.height()), (99, 99));
        assert_ne!(
            (out.width(), out.height()),
            (100, 99),
            "near-equal ratio must crop, not no-op"
        );
    }

    // ---- 6.7 boundaries -----------------------------------------------------

    #[test]
    fn upscaling_is_allowed() {
        // 4×4 source upscaled to 50×50: allowed, 2500 cells.
        let src = RgbImage::new(4, 4);
        let grid = resize_image(&src, 50, 50, &ResizeOptions::default()).unwrap();
        assert_eq!(grid.pixels.len(), 2500);
        assert_eq!((grid.width, grid.height), (50, 50));
    }

    #[test]
    fn zero_width_rejected() {
        let src = RgbImage::new(10, 10);
        let err = resize_image(&src, 0, 10, &ResizeOptions::default()).unwrap_err();
        match &err {
            BeadError::InvalidImage { reason } => {
                assert!(reason.contains("width"), "reason names width: {reason}");
            }
            other => panic!("expected InvalidImage, got {other:?}"),
        }
        // Reason is deterministic: same input → byte-for-byte identical reason.
        let err2 = resize_image(&src, 0, 10, &ResizeOptions::default()).unwrap_err();
        assert_eq!(err.to_string(), err2.to_string());
    }

    #[test]
    fn zero_height_rejected() {
        let src = RgbImage::new(10, 10);
        let err = resize_image(&src, 10, 0, &ResizeOptions::default()).unwrap_err();
        match &err {
            BeadError::InvalidImage { reason } => {
                assert!(reason.contains("height"), "reason names height: {reason}");
            }
            other => panic!("expected InvalidImage, got {other:?}"),
        }
        let err2 = resize_image(&src, 10, 0, &ResizeOptions::default()).unwrap_err();
        assert_eq!(err.to_string(), err2.to_string());
    }

    #[test]
    fn one_by_one_target_ok() {
        let grid = image_to_grid(GRADIENT_PNG, 1, 1, &ResizeOptions::default()).unwrap();
        assert_eq!(grid.pixels.len(), 1);
        assert_eq!((grid.width, grid.height), (1, 1));
    }

    #[test]
    fn image_to_grid_zero_target_rejected() {
        // The reason must be image_to_grid's OWN pre-decode guard ("target width
        // is 0"), not crop_center's later "crop target width is 0" — this pins the
        // D5′ ordering: target validated before decode/crop, not after.
        let e0 = image_to_grid(GRADIENT_PNG, 0, 10, &ResizeOptions::default()).unwrap_err();
        match e0 {
            BeadError::InvalidImage { reason } => assert_eq!(reason, "target width is 0"),
            other => panic!("expected InvalidImage, got {other:?}"),
        }
        let e1 = image_to_grid(GRADIENT_PNG, 10, 0, &ResizeOptions::default()).unwrap_err();
        match e1 {
            BeadError::InvalidImage { reason } => assert_eq!(reason, "target height is 0"),
            other => panic!("expected InvalidImage, got {other:?}"),
        }
    }

    // --- 6.7 direct-primitive self-guard tests (not via the wrapper) ---------

    #[test]
    fn crop_center_zero_target_rejected() {
        let img = decode_image(GRADIENT_PNG).unwrap();
        // Direct call: zero target dimension must reject, never divide-by-zero.
        let e0 = crop_center(&img, 0, 10).unwrap_err();
        assert!(matches!(e0, BeadError::InvalidImage { .. }));
        let e1 = crop_center(&img, 10, 0).unwrap_err();
        assert!(matches!(e1, BeadError::InvalidImage { .. }));
    }

    #[test]
    fn crop_center_extreme_ratio_rejected() {
        // 100×1 source against a 1:100 target → crop width floors to 0. And the
        // mirror: 1×100 against 100:1. Each rejects (reason names the degenerate
        // dimension), producing no all-black grid and no panic.
        let wide = RgbImage::new(100, 1);
        let e0 = crop_center(&wide, 1, 100).unwrap_err();
        match e0 {
            BeadError::InvalidImage { reason } => assert!(reason.contains("width")),
            other => panic!("expected InvalidImage, got {other:?}"),
        }
        let tall = RgbImage::new(1, 100);
        let e1 = crop_center(&tall, 100, 1).unwrap_err();
        match e1 {
            BeadError::InvalidImage { reason } => assert!(reason.contains("height")),
            other => panic!("expected InvalidImage, got {other:?}"),
        }
    }

    #[test]
    fn resize_image_zero_source_rejected() {
        // Hand-built zero-dimension source must reject before imageops::resize
        // (which would silently return Ok + an all-black grid).
        let z0 = RgbImage::new(0, 5);
        let e0 = resize_image(&z0, 80, 100, &ResizeOptions::default()).unwrap_err();
        assert!(matches!(e0, BeadError::InvalidImage { .. }));
        let z1 = RgbImage::new(5, 0);
        let e1 = resize_image(&z1, 80, 100, &ResizeOptions::default()).unwrap_err();
        assert!(matches!(e1, BeadError::InvalidImage { .. }));
    }

    #[test]
    fn crop_center_extreme_aspect_no_overflow() {
        // Small images, extreme ratios: the cross product 100000*100000 == 1e10
        // overflows u32. The u64 widening must keep this from panicking in debug
        // or wrapping in release; the result is a clean InvalidImage either way.
        let wide = RgbImage::new(100_000, 1);
        let e0 = crop_center(&wide, 1, 100_000).unwrap_err();
        assert!(matches!(e0, BeadError::InvalidImage { .. }));
        let tall = RgbImage::new(1, 100_000);
        let e1 = crop_center(&tall, 100_000, 1).unwrap_err();
        assert!(matches!(e1, BeadError::InvalidImage { .. }));
    }

    // ---- 6.8 EXIF orientation is not auto-applied ---------------------------

    #[test]
    fn exif_orientation_not_applied() {
        // The fixture is an 8×4 JPEG carrying an EXIF Orientation=6 tag. Decoding
        // at the STORED orientation must return 8×4 (unrotated). If decoding ever
        // starts auto-applying EXIF, this would flip to 4×8 and fail loudly.
        let img = decode_image(EXIF_JPG).unwrap();
        assert_eq!(
            (img.width(), img.height()),
            (8, 4),
            "EXIF orientation must not be auto-applied (stored size is 8×4)"
        );
    }

    // ---- 6.9 default filter --------------------------------------------------

    #[test]
    fn default_filter_is_lanczos3() {
        assert_eq!(ResizeOptions::default().filter, FilterType::Lanczos3);
    }
}
