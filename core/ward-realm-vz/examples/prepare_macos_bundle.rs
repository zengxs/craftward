// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc;

use ward_realm_vz::{MacOsBundleRequest, prepare_macos_bundle};

fn absolute_path(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    let path = path.as_ref();
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}

fn run() -> Result<(), String> {
    let mut arguments = env::args_os().skip(1);
    let restore_image = arguments.next().ok_or_else(|| {
        "usage: prepare_macos_bundle <restore-image.ipsw> <destination> [disk-size-gib]".to_owned()
    })?;
    let destination = arguments.next().ok_or_else(|| {
        "usage: prepare_macos_bundle <restore-image.ipsw> <destination> [disk-size-gib]".to_owned()
    })?;
    let disk_size_gib = arguments
        .next()
        .map(|value| {
            value
                .to_string_lossy()
                .parse::<u64>()
                .map_err(|error| format!("invalid disk size: {error}"))
        })
        .transpose()?
        .unwrap_or(64);
    if arguments.next().is_some() {
        return Err("too many arguments".to_owned());
    }

    let disk_size = disk_size_gib
        .checked_mul(1024 * 1024 * 1024)
        .ok_or_else(|| "disk size is too large".to_owned())?;
    let request = MacOsBundleRequest::new(
        absolute_path(restore_image).map_err(|error| error.to_string())?,
        absolute_path(destination).map_err(|error| error.to_string())?,
    )
    .with_disk_size(disk_size);
    let (sender, receiver) = mpsc::sync_channel(1);

    prepare_macos_bundle(request, move |result| {
        let _ = sender.send(result);
    })
    .map_err(|error| error.to_string())?;

    let bundle = receiver
        .recv()
        .map_err(|error| format!("bundle preparation callback failed: {error}"))?
        .map_err(|error| error.to_string())?;
    println!("Prepared {}", bundle.path.display());
    println!(
        "macOS {}.{}.{} ({})",
        bundle.operating_system_version.major,
        bundle.operating_system_version.minor,
        bundle.operating_system_version.patch,
        bundle.build_version
    );
    println!(
        "Minimum resources: {} CPUs, {} bytes of memory",
        bundle.minimum_cpu_count, bundle.minimum_memory_size
    );

    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
