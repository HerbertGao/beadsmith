use clap::Parser;

/// Beadsmith CLI — turns images into bead patterns. Thin wrapper over
/// `bead-core`; contains no algorithms. Subcommands land in M6 (see ROADMAP).
#[derive(Parser)]
#[command(name = "bead-cli", version)]
struct Cli;

fn main() -> anyhow::Result<()> {
    // ponytail: parse only proves clap is wired; real subcommands arrive in M6.
    let _ = Cli::parse();
    println!("bead-cli: workspace alive — no commands yet (see ROADMAP M6)");
    Ok(())
}
