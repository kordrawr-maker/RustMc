mod api;
mod archive;
mod config;
mod net;
mod prompt;
mod run;
mod setup;
mod stats;
mod version;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rustmc", version, about = "Minimal Minecraft server launcher and manager")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Download and configure a Minecraft server
    Setup {
        /// Wipe an existing server folder
        #[arg(long)]
        force: bool,
        /// Print the install plan without downloading anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Start the configured server with a live console
    Run,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Setup { force, dry_run } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(setup::run(force, dry_run))
        }
        Cmd::Run => run::run(),
    }
}
