// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc;

use ward_realm_vz::{MacOsBundleInstallationRequest, install_macos_bundle};

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
        "usage: install_macos_bundle <restore-image.ipsw> <prepared-bundle>".to_owned()
    })?;
    let bundle = arguments.next().ok_or_else(|| {
        "usage: install_macos_bundle <restore-image.ipsw> <prepared-bundle>".to_owned()
    })?;
    if arguments.next().is_some() {
        return Err("too many arguments".to_owned());
    }

    let request = MacOsBundleInstallationRequest::new(
        absolute_path(restore_image).map_err(|error| error.to_string())?,
        absolute_path(bundle).map_err(|error| error.to_string())?,
    );
    let (sender, receiver) = mpsc::sync_channel(1);
    let last_percentage = Arc::new(AtomicU8::new(u8::MAX));
    let progress_percentage = Arc::clone(&last_percentage);

    install_macos_bundle(
        request,
        move |fraction_completed| {
            let percentage = (fraction_completed * 100.0).round() as u8;
            if progress_percentage.swap(percentage, Ordering::Relaxed) != percentage {
                eprintln!("Installing macOS: {percentage}%");
            }
        },
        move |result| {
            let _ = sender.send(result);
        },
    )
    .map_err(|error| error.to_string())?;

    receiver
        .recv()
        .map_err(|error| format!("macOS installation callback failed: {error}"))?
        .map_err(|error| error.to_string())?;
    println!("macOS installation completed");
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
