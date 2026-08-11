// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

#[cfg(target_os = "macos")]
use super::path_to_c_string;
#[cfg(target_os = "macos")]
use crate::ffi;

/// A writable disk attached to a VZ macOS virtual machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacOsDiskConfiguration {
    pub path: PathBuf,
}

/// The graphics display exposed to a VZ macOS virtual machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MacOsDisplayConfiguration {
    pub width_pixels: u64,
    pub height_pixels: u64,
    pub pixels_per_inch: u64,
}

/// Everything required to reconstruct a VZ macOS machine configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacOsVirtualMachineConfiguration {
    pub cpu_count: u64,
    pub memory_size: u64,
    pub disks: Vec<MacOsDiskConfiguration>,
    pub auxiliary_storage: PathBuf,
    pub hardware_model: Vec<u8>,
    pub machine_identifier: Vec<u8>,
    pub display: MacOsDisplayConfiguration,
    pub mac_address: String,
}

/// Explicit files used to save and consume a VZ machine state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacOsSavedStateFiles {
    pub machine_state: PathBuf,
    pub saving: PathBuf,
    pub restoring: PathBuf,
}

#[cfg(target_os = "macos")]
pub(super) struct NativeMacOsVirtualMachineConfiguration {
    disk_paths: Vec<std::ffi::CString>,
    disks: Vec<ffi::WardVzMacOsDiskConfiguration>,
    auxiliary_storage: std::ffi::CString,
    mac_address: std::ffi::CString,
}

#[cfg(target_os = "macos")]
impl NativeMacOsVirtualMachineConfiguration {
    pub(super) fn new(
        configuration: &MacOsVirtualMachineConfiguration,
    ) -> Result<Self, &'static str> {
        if configuration.cpu_count == 0 {
            return Err("cpu count");
        }
        if configuration.memory_size == 0 {
            return Err("memory size");
        }
        if configuration.disks.is_empty() {
            return Err("disk list");
        }
        if configuration.hardware_model.is_empty() {
            return Err("hardware model");
        }
        if configuration.machine_identifier.is_empty() {
            return Err("machine identifier");
        }
        if configuration.display.width_pixels == 0
            || configuration.display.height_pixels == 0
            || configuration.display.pixels_per_inch == 0
        {
            return Err("display");
        }

        let disk_paths = configuration
            .disks
            .iter()
            .map(|disk| path_to_c_string(&disk.path, "disk", true))
            .collect::<Result<Vec<_>, _>>()?;
        let disks = disk_paths
            .iter()
            .map(|path| ffi::WardVzMacOsDiskConfiguration {
                path: path.as_ptr(),
            })
            .collect();
        let auxiliary_storage =
            path_to_c_string(&configuration.auxiliary_storage, "auxiliary storage", true)?;
        let mac_address = std::ffi::CString::new(configuration.mac_address.as_bytes())
            .map_err(|_| "MAC address")?;

        Ok(Self {
            disk_paths,
            disks,
            auxiliary_storage,
            mac_address,
        })
    }

    pub(super) fn with_raw<R>(
        &self,
        configuration: &MacOsVirtualMachineConfiguration,
        operation: impl FnOnce(&ffi::WardVzMacOsVirtualMachineConfiguration) -> R,
    ) -> R {
        debug_assert_eq!(self.disk_paths.len(), self.disks.len());
        let raw = ffi::WardVzMacOsVirtualMachineConfiguration {
            cpu_count: configuration.cpu_count,
            memory_size: configuration.memory_size,
            disks: self.disks.as_ptr(),
            disk_count: self.disks.len(),
            auxiliary_storage_path: self.auxiliary_storage.as_ptr(),
            hardware_model: ffi::WardVzByteSlice {
                data: configuration.hardware_model.as_ptr(),
                length: configuration.hardware_model.len(),
            },
            machine_identifier: ffi::WardVzByteSlice {
                data: configuration.machine_identifier.as_ptr(),
                length: configuration.machine_identifier.len(),
            },
            display_width: configuration.display.width_pixels,
            display_height: configuration.display.height_pixels,
            display_pixels_per_inch: configuration.display.pixels_per_inch,
            mac_address: self.mac_address.as_ptr(),
        };
        operation(&raw)
    }
}

#[cfg(target_os = "macos")]
pub(super) struct NativeMacOsSavedStateFiles {
    pub(super) machine_state: std::ffi::CString,
    pub(super) saving: std::ffi::CString,
    pub(super) restoring: std::ffi::CString,
}

#[cfg(target_os = "macos")]
impl NativeMacOsSavedStateFiles {
    pub(super) fn new(files: &MacOsSavedStateFiles) -> Result<Self, &'static str> {
        Ok(Self {
            machine_state: path_to_c_string(&files.machine_state, "machine state", true)?,
            saving: path_to_c_string(&files.saving, "saving machine state", true)?,
            restoring: path_to_c_string(&files.restoring, "restoring machine state", true)?,
        })
    }
}
