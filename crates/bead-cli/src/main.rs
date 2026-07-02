//! Beadsmith CLI — turns images into bead patterns. Thin wrapper over
//! `bead-core`; contains **no algorithms**. All filesystem/IO and `anyhow`
//! context live here (CLAUDE rule 1); `bead-core` stays fs-free.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bead_core::pipeline::pattern_json;
use bead_core::{generate_pattern, load_palette, GenerateOptions, MatcherKind};
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
        /// Output directory (created if missing; files are overwritten).
        #[arg(long)]
        output: PathBuf,
        /// Limit the pattern to at most N bead colors (e.g. 24/36/48/72).
        #[arg(long)]
        max_colors: Option<u32>,
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
    /// List available palettes (not implemented in M6).
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Generate {
            input,
            palette,
            width,
            height,
            matcher,
            output,
            max_colors,
        } => generate(
            &input, &palette, width, height, matcher, &output, max_colors,
        ),
        Command::Palette { command } => match command {
            PaletteCmd::Validate { path } => palette_validate(&path),
            // ponytail: 桩成显式非零退出，不假绿、不 panic
            PaletteCmd::List => {
                anyhow::bail!("palette list: coming soon (not implemented in M6)")
            }
        },
        // ponytail: 桩成显式非零退出，不假绿、不 panic
        Command::Inspect { .. } => {
            anyhow::bail!("inspect: coming soon (not implemented in M6)")
        }
    }
}

fn generate(
    input: &Path,
    palette: &Path,
    width: u32,
    height: u32,
    matcher: CliMatcher,
    output: &Path,
    max_colors: Option<u32>,
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
        max_colors,
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

fn palette_validate(path: &Path) -> Result<()> {
    let bytes = fs::read(path).with_context(|| format!("failed to read palette {path:?}"))?;
    load_palette(&bytes).with_context(|| format!("invalid palette {path:?}"))?;
    println!("palette {path:?} is valid");
    Ok(())
}
