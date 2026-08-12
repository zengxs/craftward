// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

mod licenses;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(about = "Craftward repository tasks", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Manage bundled application and third-party license resources.
    Licenses(LicensesArgs),
}

#[derive(Debug, Args)]
struct LicensesArgs {
    #[command(subcommand)]
    command: LicensesCommand,
}

#[derive(Debug, Subcommand)]
enum LicensesCommand {
    /// Download pinned remote license texts into the committed cache.
    Fetch {
        /// Path to the license manifest.
        #[arg(long, value_name = "PATH")]
        manifest: Option<PathBuf>,
    },
    /// Generate the legal catalog and Qt resource collection without network access.
    Generate {
        /// Path to the license manifest.
        #[arg(long, value_name = "PATH")]
        manifest: Option<PathBuf>,

        /// Directory that receives generated legal resources.
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,
    },
    /// Validate the manifest and every local or cached license source.
    Check {
        /// Path to the license manifest.
        #[arg(long, value_name = "PATH")]
        manifest: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let defaults = licenses::ProjectPaths::default();

    match cli.command {
        Command::Licenses(args) => match args.command {
            LicensesCommand::Fetch { manifest } => {
                let manifest = manifest.unwrap_or(defaults.manifest);
                let fetched = licenses::fetch(&manifest, &defaults.cache_dir)?;
                println!("Fetched {fetched} license sources.");
            }
            LicensesCommand::Generate { manifest, output } => {
                let manifest = manifest.unwrap_or(defaults.manifest);
                let output = output.unwrap_or(defaults.output_dir);
                let generated = licenses::generate(&manifest, &defaults.cache_dir, &output)?;
                println!(
                    "Generated {generated} license documents in {}.",
                    output.display()
                );
            }
            LicensesCommand::Check { manifest } => {
                let manifest = manifest.unwrap_or(defaults.manifest);
                let checked = licenses::check(&manifest, &defaults.cache_dir)?;
                println!("Checked {checked} license documents.");
            }
        },
    }

    Ok(())
}
