// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
use super::{copy_bridge_bytes, copy_bridge_string, is_supported, path_to_c_string};
#[cfg(target_os = "macos")]
use crate::ffi;

/// Explicit files and capacity required to prepare a VZ macOS guest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacOsPreparationRequest {
    pub restore_image: PathBuf,
    pub disk: PathBuf,
    pub auxiliary_storage: PathBuf,
    pub disk_size: u64,
}

/// A semantic macOS version reported by a restore image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MacOsVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

/// Backend metadata produced while preparing VZ macOS files.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacOsPreparationInfo {
    pub build_version: String,
    pub operating_system_version: MacOsVersion,
    pub minimum_cpu_count: u64,
    pub minimum_memory_size: u64,
    pub hardware_model: Vec<u8>,
    pub machine_identifier: Vec<u8>,
    pub mac_address: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MacOsPreparationError {
    UnsupportedHost,
    InvalidPath(&'static str),
    InvalidDiskSize(u64),
    Native {
        domain: String,
        code: i64,
        message: String,
    },
}

impl fmt::Display for MacOsPreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedHost => formatter
                .write_str("this host cannot prepare a Virtualization.framework macOS guest"),
            Self::InvalidPath(name) => write!(formatter, "the {name} path is invalid"),
            Self::InvalidDiskSize(size) => {
                write!(
                    formatter,
                    "the requested disk size ({size} bytes) is invalid"
                )
            }
            Self::Native {
                domain,
                code,
                message,
            } => write!(formatter, "{message} ({domain}, code {code})"),
        }
    }
}

impl std::error::Error for MacOsPreparationError {}

/// Prepares backend files without creating or parsing a Realm manifest.
#[cfg(target_os = "macos")]
pub fn prepare_macos(
    request: MacOsPreparationRequest,
    completion: impl FnOnce(Result<MacOsPreparationInfo, MacOsPreparationError>) + Send + 'static,
) -> Result<(), MacOsPreparationError> {
    use std::ffi::c_void;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    type Completion =
        Box<dyn FnOnce(Result<MacOsPreparationInfo, MacOsPreparationError>) + Send + 'static>;

    struct PreparationContext {
        completion: Option<Completion>,
    }

    unsafe extern "C" fn preparation_completed(
        context: *mut c_void,
        preparation_info: *const ffi::WardVzMacOsPreparationInfo,
        error: *const ffi::WardVzError,
    ) {
        if context.is_null() {
            return;
        }
        // SAFETY: This pointer was produced by Box::into_raw below, and the
        // bridge guarantees exactly one completion call.
        let mut context = unsafe { Box::from_raw(context.cast::<PreparationContext>()) };
        let result = if !error.is_null() {
            // SAFETY: Bridge values remain valid for this callback.
            let error = unsafe { &*error };
            Err(MacOsPreparationError::Native {
                // SAFETY: Bridge strings remain valid for this callback.
                domain: unsafe { copy_bridge_string(error.domain) },
                code: error.code,
                // SAFETY: Bridge strings remain valid for this callback.
                message: unsafe { copy_bridge_string(error.message) },
            })
        } else if preparation_info.is_null() {
            Err(missing_result_error())
        } else {
            // SAFETY: The preparation info and its fields remain valid for
            // this callback.
            let info = unsafe { &*preparation_info };
            let hardware_model = unsafe { copy_bridge_bytes(info.hardware_model) };
            let machine_identifier = unsafe { copy_bridge_bytes(info.machine_identifier) };
            match (hardware_model, machine_identifier) {
                (Some(hardware_model), Some(machine_identifier)) => Ok(MacOsPreparationInfo {
                    // SAFETY: Bridge strings remain valid for this callback.
                    build_version: unsafe { copy_bridge_string(info.build_version) },
                    operating_system_version: MacOsVersion {
                        major: info.os_version_major,
                        minor: info.os_version_minor,
                        patch: info.os_version_patch,
                    },
                    minimum_cpu_count: info.minimum_cpu_count,
                    minimum_memory_size: info.minimum_memory_size,
                    hardware_model,
                    machine_identifier,
                    // SAFETY: Bridge strings remain valid for this callback.
                    mac_address: unsafe { copy_bridge_string(info.mac_address) },
                }),
                _ => Err(missing_result_error()),
            }
        };
        if let Some(completion) = context.completion.take() {
            let _ = catch_unwind(AssertUnwindSafe(|| completion(result)));
        }
    }

    if request.disk_size == 0
        || !request.disk_size.is_multiple_of(512)
        || request.disk_size > i64::MAX as u64
    {
        return Err(MacOsPreparationError::InvalidDiskSize(request.disk_size));
    }
    let restore_image = path_to_c_string(&request.restore_image, "restore image", false)
        .map_err(MacOsPreparationError::InvalidPath)?;
    let disk = path_to_c_string(&request.disk, "disk", true)
        .map_err(MacOsPreparationError::InvalidPath)?;
    let auxiliary_storage = path_to_c_string(&request.auxiliary_storage, "auxiliary storage", true)
        .map_err(MacOsPreparationError::InvalidPath)?;
    if !is_supported() {
        return Err(MacOsPreparationError::UnsupportedHost);
    }

    let context = Box::new(PreparationContext {
        completion: Some(Box::new(completion)),
    });
    // SAFETY: The bridge copies all paths before returning and owns the
    // completion context until it invokes preparation_completed once.
    unsafe {
        ffi::ward_vz_prepare_macos(
            restore_image.as_ptr(),
            disk.as_ptr(),
            auxiliary_storage.as_ptr(),
            request.disk_size,
            preparation_completed,
            Box::into_raw(context).cast(),
        );
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn prepare_macos(
    _request: MacOsPreparationRequest,
    _completion: impl FnOnce(Result<MacOsPreparationInfo, MacOsPreparationError>) + Send + 'static,
) -> Result<(), MacOsPreparationError> {
    Err(MacOsPreparationError::UnsupportedHost)
}

#[cfg(target_os = "macos")]
fn missing_result_error() -> MacOsPreparationError {
    MacOsPreparationError::Native {
        domain: "app.craftward.ward-realm-vz.bridge".into(),
        code: -1,
        message: "the native bridge returned incomplete preparation metadata".into(),
    }
}
