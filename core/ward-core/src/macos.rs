// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use thiserror::Error;
use ward_realm::{
    DiskFormat, MIB, MacOsRestoreImage, NewVzMacOsRealm, PendingRealmBundle, RealmBundle,
    RealmBundleError, VzDisplay, VzInstallationState,
};
use ward_realm_vz::{
    MacOsDiskConfiguration, MacOsDisplayConfiguration, MacOsInstallationError,
    MacOsInstallationRequest, MacOsPreparationError, MacOsPreparationRequest, MacOsSavedStateFiles,
    MacOsVirtualMachine, MacOsVirtualMachineConfiguration, MacOsVirtualMachineError,
    MacOsVirtualMachineEvent, install_macos, prepare_macos,
};

pub use ward_realm_vz::MacOsVersion;

/// The default logical size of a new macOS system disk.
pub const DEFAULT_MACOS_DISK_SIZE: u64 = 64 * 1024 * 1024 * 1024;

const DEFAULT_DISPLAY_WIDTH: u64 = 1920;
const DEFAULT_DISPLAY_HEIGHT: u64 = 1200;
const DEFAULT_DISPLAY_PIXELS_PER_INCH: u64 = 80;

/// Parameters for preparing a Realm bundle for macOS installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacOsBundleRequest {
    pub restore_image: PathBuf,
    pub destination: PathBuf,
    pub disk_size: u64,
}

impl MacOsBundleRequest {
    pub fn new(restore_image: impl Into<PathBuf>, destination: impl Into<PathBuf>) -> Self {
        Self {
            restore_image: restore_image.into(),
            destination: destination.into(),
            disk_size: DEFAULT_MACOS_DISK_SIZE,
        }
    }

    #[must_use]
    pub const fn with_disk_size(mut self, disk_size: u64) -> Self {
        self.disk_size = disk_size;
        self
    }
}

/// Details of a newly prepared macOS Realm bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacOsBundleInfo {
    pub path: PathBuf,
    pub build_version: String,
    pub operating_system_version: MacOsVersion,
    pub minimum_cpu_count: u64,
    pub minimum_memory_size: u64,
    pub disk_size: u64,
}

/// Parameters for installing macOS into a prepared Realm bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacOsBundleInstallationRequest {
    pub restore_image: PathBuf,
    pub bundle: PathBuf,
}

impl MacOsBundleInstallationRequest {
    pub fn new(restore_image: impl Into<PathBuf>, bundle: impl Into<PathBuf>) -> Self {
        Self {
            restore_image: restore_image.into(),
            bundle: bundle.into(),
        }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MacOsBundlePreparationError {
    #[error(transparent)]
    Bundle(#[from] RealmBundleError),
    #[error(transparent)]
    Adapter(#[from] MacOsPreparationError),
    #[error("Virtualization.framework reported an invalid minimum memory size: {0} bytes")]
    InvalidMinimumMemorySize(u64),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MacOsBundleInstallationError {
    #[error(transparent)]
    Bundle(#[from] RealmBundleError),
    #[error(transparent)]
    Adapter(#[from] MacOsInstallationError),
    #[error("the Realm cannot be converted to a VZ macOS configuration: {0}")]
    InvalidConfiguration(String),
    #[error(
        "macOS installation failed ({installation_error}) and its failure state could not be recorded: {state_error}"
    )]
    InstallationFailureState {
        installation_error: MacOsInstallationError,
        state_error: RealmBundleError,
    },
    #[error(
        "macOS installation could not start ({start_error}) and the prepared state could not be restored: {state_error}"
    )]
    StartRollback {
        start_error: MacOsInstallationError,
        state_error: RealmBundleError,
    },
}

#[derive(Debug, Error)]
pub(crate) enum MacOsRealmOpenError {
    #[error(transparent)]
    Bundle(#[from] RealmBundleError),
    #[error("the Realm cannot be converted to a VZ macOS configuration: {0}")]
    InvalidConfiguration(String),
    #[error(transparent)]
    Adapter(#[from] MacOsVirtualMachineError),
}

/// Creates the machine-owned bundle structure and asks the VZ adapter to
/// populate only its explicit disk and auxiliary-storage files.
pub fn prepare_macos_bundle(
    request: MacOsBundleRequest,
    completion: impl FnOnce(Result<MacOsBundleInfo, MacOsBundlePreparationError>) + Send + 'static,
) -> Result<(), MacOsBundlePreparationError> {
    let pending = PendingRealmBundle::begin(&request.destination)?;
    let disk_size = request.disk_size;
    let adapter_request = MacOsPreparationRequest {
        restore_image: request.restore_image,
        disk: pending.system_disk_path(),
        auxiliary_storage: pending.auxiliary_storage_path(),
        disk_size,
    };

    prepare_macos(adapter_request, move |result| {
        let result = result
            .map_err(MacOsBundlePreparationError::from)
            .and_then(|info| {
                if info.minimum_memory_size == 0 || !info.minimum_memory_size.is_multiple_of(MIB) {
                    return Err(MacOsBundlePreparationError::InvalidMinimumMemorySize(
                        info.minimum_memory_size,
                    ));
                }

                let operating_system_version = info.operating_system_version;
                let build_version = info.build_version.clone();
                let minimum_cpu_count = info.minimum_cpu_count;
                let minimum_memory_size = info.minimum_memory_size;
                let metadata = NewVzMacOsRealm {
                    restore_image: MacOsRestoreImage {
                        version: format!(
                            "{}.{}.{}",
                            operating_system_version.major,
                            operating_system_version.minor,
                            operating_system_version.patch
                        ),
                        build: info.build_version,
                        minimum_cpu_count,
                        minimum_memory_mib: minimum_memory_size / MIB,
                    },
                    disk_logical_size_bytes: disk_size,
                    display: VzDisplay {
                        width_pixels: DEFAULT_DISPLAY_WIDTH,
                        height_pixels: DEFAULT_DISPLAY_HEIGHT,
                        pixels_per_inch: DEFAULT_DISPLAY_PIXELS_PER_INCH,
                    },
                    mac_address: info.mac_address,
                    hardware_model: info.hardware_model,
                    machine_identifier: info.machine_identifier,
                };
                let bundle = pending.publish_vz_macos(metadata)?;
                Ok(MacOsBundleInfo {
                    path: bundle.root().to_owned(),
                    build_version,
                    operating_system_version,
                    minimum_cpu_count,
                    minimum_memory_size,
                    disk_size,
                })
            });
        completion(result);
    })?;
    Ok(())
}

/// Installs macOS and durably records each installation-state transition in
/// the Realm manifest owned by `ward-realm`.
pub fn install_macos_bundle(
    request: MacOsBundleInstallationRequest,
    progress: impl FnMut(f64) + Send + 'static,
    completion: impl FnOnce(Result<(), MacOsBundleInstallationError>) + Send + 'static,
) -> Result<(), MacOsBundleInstallationError> {
    let mut bundle = RealmBundle::open(request.bundle)?;
    bundle.require_vz_macos_installation_state(VzInstallationState::Prepared)?;
    let (configuration, _) = configuration_from_bundle(&bundle)
        .map_err(MacOsBundleInstallationError::InvalidConfiguration)?;
    bundle.update_vz_macos_installation_state(VzInstallationState::Installing)?;

    let bundle = Arc::new(Mutex::new(bundle));
    let completion_bundle = Arc::clone(&bundle);
    let adapter_request = MacOsInstallationRequest {
        restore_image: request.restore_image,
        configuration,
    };
    let start_result = install_macos(adapter_request, progress, move |result| {
        let final_state = if result.is_ok() {
            VzInstallationState::Installed
        } else {
            VzInstallationState::InstallationFailed
        };
        let state_result = completion_bundle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .update_vz_macos_installation_state(final_state);
        let result = match (result, state_result) {
            (Ok(()), Err(error)) => Err(MacOsBundleInstallationError::Bundle(error)),
            (Err(installation_error), Err(state_error)) => {
                Err(MacOsBundleInstallationError::InstallationFailureState {
                    installation_error,
                    state_error,
                })
            }
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) => Err(MacOsBundleInstallationError::Adapter(error)),
        };
        completion(result);
    });

    if let Err(error) = start_result {
        let rollback = bundle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .update_vz_macos_installation_state(VzInstallationState::Prepared);
        return match rollback {
            Ok(()) => Err(MacOsBundleInstallationError::Adapter(error)),
            Err(state_error) => Err(MacOsBundleInstallationError::StartRollback {
                start_error: error,
                state_error,
            }),
        };
    }
    Ok(())
}

pub(crate) fn open_macos_realm(
    bundle_path: PathBuf,
    event_handler: impl FnMut(MacOsVirtualMachineEvent) + Send + 'static,
) -> Result<MacOsVirtualMachine, MacOsRealmOpenError> {
    let bundle = RealmBundle::open(bundle_path)?;
    bundle.require_vz_macos_installation_state(VzInstallationState::Installed)?;
    let (configuration, saved_state) =
        configuration_from_bundle(&bundle).map_err(MacOsRealmOpenError::InvalidConfiguration)?;
    MacOsVirtualMachine::open(configuration, saved_state, event_handler)
        .map_err(MacOsRealmOpenError::Adapter)
}

fn configuration_from_bundle(
    bundle: &RealmBundle,
) -> Result<(MacOsVirtualMachineConfiguration, MacOsSavedStateFiles), String> {
    let resolved = bundle
        .resolve_vz_macos()
        .map_err(|error| error.to_string())?;
    let memory_size = resolved
        .memory_mib
        .checked_mul(MIB)
        .ok_or_else(|| "memory_mib cannot be represented in bytes".to_owned())?;
    let disks = resolved
        .disks
        .into_iter()
        .map(|disk| match disk.format {
            DiskFormat::Raw | DiskFormat::Asif => Ok(MacOsDiskConfiguration { path: disk.path }),
            DiskFormat::Qcow2 => Err("VZ cannot attach a qcow2 disk directly".to_owned()),
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok((
        MacOsVirtualMachineConfiguration {
            cpu_count: resolved.cpu_count,
            memory_size,
            disks,
            auxiliary_storage: resolved.auxiliary_storage,
            hardware_model: resolved.hardware_model,
            machine_identifier: resolved.machine_identifier,
            display: MacOsDisplayConfiguration {
                width_pixels: resolved.display.width_pixels,
                height_pixels: resolved.display.height_pixels,
                pixels_per_inch: resolved.display.pixels_per_inch,
            },
            mac_address: resolved.mac_address,
        },
        MacOsSavedStateFiles {
            machine_state: resolved.saved_state.machine_state,
            saving: resolved.saved_state.saving,
            restoring: resolved.saved_state.restoring,
        },
    ))
}
