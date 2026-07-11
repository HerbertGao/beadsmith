//! Beadsmith CLI — turns images into bead patterns. Thin wrapper over
//! `bead-core`; contains **no algorithms**. All filesystem/IO and `anyhow`
//! context live here (CLAUDE rule 1); `bead-core` stays fs-free.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bead_core::pipeline::pattern_json;
use bead_core::{generate_pattern, load_palette, GenerateOptions, GeneratorKind, MatcherKind};
use clap::{Parser, Subcommand, ValueEnum};

/// Beadsmith CLI — turns images into bead patterns. Thin wrapper over
/// `bead-core`; contains no algorithms.
#[derive(Parser)]
#[command(name = "bead-cli", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a bead pattern from an image and write the four output files.
    Generate {
        /// Source image path.
        #[arg(long)]
        input: PathBuf,
        /// Palette JSON path.
        #[arg(long)]
        palette: PathBuf,
        /// Target grid width in cells.
        #[arg(long)]
        width: u32,
        /// Target grid height in cells.
        #[arg(long)]
        height: u32,
        /// Matching strategy: rgb, lab, or oklab.
        #[arg(long, value_enum, default_value = "oklab")]
        matcher: CliMatcher,
        /// Generation mode: staged (default) or gerstner (photo path).
        #[arg(long, value_enum, default_value = "staged")]
        generator: CliGenerator,
        /// Output directory (created if missing; files are overwritten).
        #[arg(long)]
        output: PathBuf,
        /// Limit the pattern to at most N bead colors (e.g. 24/36/48/72).
        #[arg(long)]
        max_colors: Option<u32>,
        /// Clean up isolated same-color regions of at most N beads (0 = no-op).
        #[arg(long)]
        despeckle: Option<u32>,
    },
    /// Palette subcommands.
    Palette {
        #[command(subcommand)]
        command: PaletteCmd,
    },
    /// Inspect an existing pattern (not implemented in M6).
    Inspect {
        /// Path to inspect.
        path: PathBuf,
    },
}

#[derive(Subcommand)]
enum PaletteCmd {
    /// Validate a palette JSON file.
    Validate {
        /// Palette JSON path.
        path: PathBuf,
    },
    /// List the built-in palettes (id, brand, color count).
    List,
}

/// Built-in palettes embedded at compile time (not an fs scan → self-contained
/// binary). The set is drift-guarded by `builtin_palettes_match_source_dir`; array
/// order (= `palette list` display order) mirrors the App's registry by hand, not test.
const BUILTIN_PALETTES: &[(&str, &str)] = &[
    ("mard", include_str!("../../../palettes/mard.json")),
    ("artkal_s", include_str!("../../../palettes/artkal_s.json")),
    ("artkal_a", include_str!("../../../palettes/artkal_a.json")),
    ("artkal_c", include_str!("../../../palettes/artkal_c.json")),
    ("artkal_m", include_str!("../../../palettes/artkal_m.json")),
    ("artkal_r", include_str!("../../../palettes/artkal_r.json")),
    ("hama", include_str!("../../../palettes/hama.json")),
    (
        "hama_maxi",
        include_str!("../../../palettes/hama_maxi.json"),
    ),
    (
        "hama_mini",
        include_str!("../../../palettes/hama_mini.json"),
    ),
    ("perler", include_str!("../../../palettes/perler.json")),
    (
        "perler_caps",
        include_str!("../../../palettes/perler_caps.json"),
    ),
    (
        "perler_mini",
        include_str!("../../../palettes/perler_mini.json"),
    ),
    ("nabbi", include_str!("../../../palettes/nabbi.json")),
    ("yant", include_str!("../../../palettes/yant.json")),
];

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Generate {
            input,
            palette,
            width,
            height,
            matcher,
            generator,
            output,
            max_colors,
            despeckle,
        } => generate(
            &input, &palette, width, height, matcher, generator, &output, max_colors, despeckle,
        ),
        Command::Palette { command } => match command {
            PaletteCmd::Validate { path } => palette_validate(&path),
            PaletteCmd::List => palette_list(),
        },
        // ponytail: 桩成显式非零退出，不假绿、不 panic
        Command::Inspect { .. } => {
            anyhow::bail!("inspect: coming soon (not implemented in M6)")
        }
    }
}

// ponytail: thin CLI passthrough — flags map 1:1 to GenerateOptions; a params
// struct here would be pure ceremony. Group them if a 9th flag ever lands.
#[allow(clippy::too_many_arguments)]
fn generate(
    input: &Path,
    palette: &Path,
    width: u32,
    height: u32,
    matcher: CliMatcher,
    generator: CliGenerator,
    output: &Path,
    max_colors: Option<u32>,
    despeckle: Option<u32>,
) -> Result<()> {
    let img_bytes =
        fs::read(input).with_context(|| format!("failed to read input image {input:?}"))?;
    let pal_bytes =
        fs::read(palette).with_context(|| format!("failed to read palette {palette:?}"))?;
    let palette_data =
        load_palette(&pal_bytes).with_context(|| format!("invalid palette {palette:?}"))?;

    let opts = GenerateOptions {
        width,
        height,
        matcher: matcher.into(),
        generator: generator.into(),
        max_colors,
        despeckle,
        ..Default::default()
    };
    let result =
        generate_pattern(&img_bytes, &palette_data, &opts).context("failed to generate pattern")?;

    fs::create_dir_all(output)
        .with_context(|| format!("failed to create output dir {output:?}"))?;

    let preview_path = output.join("preview.png");
    fs::write(&preview_path, &result.preview_png)
        .with_context(|| format!("failed to write {preview_path:?}"))?;

    let grid_path = output.join("grid.png");
    fs::write(&grid_path, &result.grid_png)
        .with_context(|| format!("failed to write {grid_path:?}"))?;

    let json_path = output.join("pattern.json");
    fs::write(&json_path, pattern_json(&result))
        .with_context(|| format!("failed to write {json_path:?}"))?;

    let summary_path = output.join("summary.txt");
    fs::write(&summary_path, &result.summary)
        .with_context(|| format!("failed to write {summary_path:?}"))?;

    Ok(())
}

#[derive(ValueEnum, Debug, Clone, Copy)]
enum CliMatcher {
    Rgb,
    Lab,
    Oklab,
}

impl From<CliMatcher> for MatcherKind {
    fn from(v: CliMatcher) -> Self {
        match v {
            CliMatcher::Rgb => Self::Rgb,
            CliMatcher::Lab => Self::Lab,
            CliMatcher::Oklab => Self::Oklab,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy)]
enum CliGenerator {
    Staged,
    Gerstner,
}

impl From<CliGenerator> for GeneratorKind {
    fn from(v: CliGenerator) -> Self {
        match v {
            CliGenerator::Staged => Self::Staged,
            CliGenerator::Gerstner => Self::Gerstner,
        }
    }
}

fn palette_validate(path: &Path) -> Result<()> {
    let bytes = fs::read(path).with_context(|| format!("failed to read palette {path:?}"))?;
    load_palette(&bytes).with_context(|| format!("invalid palette {path:?}"))?;
    println!("palette {path:?} is valid");
    Ok(())
}

fn palette_list() -> Result<()> {
    for &(id, json) in BUILTIN_PALETTES {
        let pal = load_palette(json.as_bytes())
            .with_context(|| format!("built-in palette {id:?} failed to parse"))?;
        // ponytail: {:<12}/{:<13} is min-width (longest id/brand today = 11); a
        // ≥12-char future id misaligns but never truncates — widen the fields then.
        println!("{:<12} {:<13} {} colors", id, pal.brand, pal.colors.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_palettes_all_parse() {
        // 14 is a deliberate tripwire (hand-bump when a palette lands); it also
        // catches a duplicate id, which match_source_dir's BTreeSet would dedup away.
        assert_eq!(BUILTIN_PALETTES.len(), 14);
        for &(id, json) in BUILTIN_PALETTES {
            let pal = load_palette(json.as_bytes())
                .unwrap_or_else(|e| panic!("built-in palette {id} failed: {e}"));
            assert!(
                !pal.colors.is_empty(),
                "built-in palette {id} has no colors"
            );
        }
    }

    /// Guards the built-in set against drifting from the source `palettes/` dir —
    /// the CLI-side analogue of the App's `palette_assets_test` (design D8). Locks
    /// identity, not just count: a palette added/removed on disk, or a mistyped id
    /// (wrong `include_str!` basename), fails here instead of silently shipping.
    #[test]
    fn builtin_palettes_match_source_dir() {
        use std::collections::BTreeSet;
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../palettes");
        // Top-level *.json only — the `_unlicensed/` subdir (AGPL) and README.md
        // are skipped by extension and must never be embedded.
        let disk: BTreeSet<String> = fs::read_dir(dir)
            .expect("read palettes dir")
            .map(|e| e.expect("dir entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
            .collect();
        let ids: BTreeSet<String> = BUILTIN_PALETTES
            .iter()
            .map(|&(id, _)| id.to_string())
            .collect();
        assert_eq!(
            ids, disk,
            "BUILTIN_PALETTES must equal palettes/*.json stems (add/remove in sync)"
        );
        // Each embedded JSON must be byte-equal to palettes/<id>.json — ties the id
        // label to the file it claims (catches a wrong include_str! basename).
        for &(id, json) in BUILTIN_PALETTES {
            let path = format!("{dir}/{id}.json");
            let on_disk = fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
            assert_eq!(
                json.as_bytes(),
                on_disk.as_slice(),
                "{id}: embedded JSON must match {path}"
            );
        }
    }
}
