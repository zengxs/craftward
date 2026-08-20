// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

mod bindings;
mod licenses;
mod version;

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
    /// Manage the generated Ward Core C interface.
    Bindings(BindingsArgs),

    /// Manage bundled application and third-party license resources.
    Licenses(LicensesArgs),

    /// Manage the synchronized Craftward product version.
    Version(VersionArgs),
}

#[derive(Debug, Args)]
struct BindingsArgs {
    #[command(subcommand)]
    command: BindingsCommand,
}

#[derive(Debug, Subcommand)]
enum BindingsCommand {
    /// Generate the committed Ward Core C header from its Rust interface.
    Generate,

    /// Check that the committed Ward Core C header matches its Rust interface.
    Check,
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

#[derive(Debug, Args)]
struct VersionArgs {
    #[command(subcommand)]
    command: VersionCommand,
}

#[derive(Debug, Subcommand)]
enum VersionCommand {
    /// Synchronize Cargo and CMake with version.toml.
    Sync,

    /// Check that Cargo and CMake match version.toml without changing files.
    Check,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Bindings(args) => {
            let paths = bindings::ProjectPaths::default();
            match args.command {
                BindingsCommand::Generate => {
                    if bindings::generate(&paths)? {
                        println!(
                            "Generated the Ward Core C interface at {}.",
                            paths.header.display()
                        );
                    } else {
                        println!("The Ward Core C interface is already synchronized.");
                    }
                }
                BindingsCommand::Check => {
                    bindings::check(&paths)?;
                    println!("The Ward Core C interface is synchronized.");
                }
            }
        }
        Command::Licenses(args) => {
            let defaults = licenses::ProjectPaths::default();
            match args.command {
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
            }
        }
        Command::Version(args) => {
            let paths = version::ProjectPaths::default();
            match args.command {
                VersionCommand::Sync => {
                    let product = version::sync(&paths)?;
                    println!("Synchronized product version {product}.");
                }
                VersionCommand::Check => {
                    let product = version::check(&paths)?;
                    println!("Product version {product} is synchronized.");
                }
            }
        }
    }

    Ok(())
}
