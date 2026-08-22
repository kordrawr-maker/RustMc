pub mod adoptium;
pub mod fabric;
pub mod neoforge;
pub mod paper;
pub mod vanilla;

use anyhow::{bail, Context, Result};
use std::path::Path;

pub fn run_java(java: &Path, cwd: &Path, args: &[String]) -> Result<()> {
    println!("  java {}", args.join(" "));
    let status = std::process::Command::new(java)
        .args(args)
        .current_dir(cwd)
        .status()
        .context("failed to launch java")?;
    if !status.success() {
        bail!("java exited with {status}");
    }
    Ok(())
}
