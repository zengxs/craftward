// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashSet;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use uuid::Uuid;

use crate::RealmValidationError;

/// The number of bytes represented by one mebibyte in a Realm manifest.
pub const MIB: u64 = 1024 * 1024;

/// A validated, UTF-8, bundle-relative artifact path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelativePath(PathBuf);

impl RelativePath {
    pub fn new(value: impl AsRef<str>) -> Result<Self, RealmValidationError> {
        let value = value.as_ref();
        let path = Path::new(value);
        let valid = !value.is_empty()
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_)));
        if !valid {
            return Err(RealmValidationError::InvalidRelativePath(value.to_owned()));
        }
        Ok(Self(path.to_owned()))
    }

    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0
            .to_str()
            .expect("RelativePath is constructed from a UTF-8 string")
    }
}

impl fmt::Display for RelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealmKind {
    VirtualMachine,
    Container,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuestOperatingSystem {
    MacOs,
    Linux,
    Windows,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Architecture {
    Aarch64,
    X86_64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Platform {
    pub operating_system: GuestOperatingSystem,
    pub architecture: Architecture,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiskFormat {
    Raw,
    Qcow2,
    Asif,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Disk {
    pub id: Uuid,
    pub path: RelativePath,
    pub format: DiskFormat,
    pub logical_size_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VzInstallationState {
    Prepared,
    Installing,
    Installed,
    InstallationFailed,
}

impl fmt::Display for VzInstallationState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Prepared => "prepared",
            Self::Installing => "installing",
            Self::Installed => "installed",
            Self::InstallationFailed => "installation_failed",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacOsRestoreImage {
    pub version: String,
    pub build: String,
    pub minimum_cpu_count: u64,
    pub minimum_memory_mib: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VzDisplay {
    pub width_pixels: u64,
    pub height_pixels: u64,
    pub pixels_per_inch: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkMode {
    Nat,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VzNetwork {
    pub mode: NetworkMode,
    pub mac_address: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VzMacOsConfiguration {
    pub installation_state: VzInstallationState,
    pub restore_image: MacOsRestoreImage,
    pub display: VzDisplay,
    pub network: VzNetwork,
    pub hardware_model: Vec<u8>,
    pub machine_identifier: Vec<u8>,
    pub auxiliary_storage: RelativePath,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RealmBackend {
    Vz(VzBackend),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VzBackend {
    MacOs(VzMacOsConfiguration),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Realm {
    pub id: Uuid,
    pub kind: RealmKind,
    pub platform: Platform,
    pub cpu_count: u64,
    pub memory_mib: u64,
    pub disks: Vec<Disk>,
    pub backend: RealmBackend,
}

/// Backend metadata used to publish a newly prepared VZ macOS Realm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewVzMacOsRealm {
    pub restore_image: MacOsRestoreImage,
    pub disk_logical_size_bytes: u64,
    pub display: VzDisplay,
    pub mac_address: String,
    pub hardware_model: Vec<u8>,
    pub machine_identifier: Vec<u8>,
}

impl Realm {
    pub(crate) fn new_vz_macos(value: NewVzMacOsRealm) -> Result<Self, RealmValidationError> {
        let realm = Self {
            id: Uuid::new_v4(),
            kind: RealmKind::VirtualMachine,
            platform: Platform {
                operating_system: GuestOperatingSystem::MacOs,
                architecture: Architecture::Aarch64,
            },
            cpu_count: value.restore_image.minimum_cpu_count,
            memory_mib: value.restore_image.minimum_memory_mib,
            disks: vec![Disk {
                id: Uuid::new_v4(),
                path: RelativePath::new("disks/disk0.img")?,
                format: DiskFormat::Raw,
                logical_size_bytes: value.disk_logical_size_bytes,
            }],
            backend: RealmBackend::Vz(VzBackend::MacOs(VzMacOsConfiguration {
                installation_state: VzInstallationState::Prepared,
                restore_image: value.restore_image,
                display: value.display,
                network: VzNetwork {
                    mode: NetworkMode::Nat,
                    mac_address: value.mac_address,
                },
                hardware_model: value.hardware_model,
                machine_identifier: value.machine_identifier,
                auxiliary_storage: RelativePath::new("auxiliary_storage.bin")?,
            })),
        };
        realm.validate()?;
        Ok(realm)
    }

    pub(crate) fn validate(&self) -> Result<(), RealmValidationError> {
        if self.id.is_nil() {
            return Err(RealmValidationError::NilIdentifier("Realm id"));
        }
        if self.cpu_count == 0 {
            return Err(RealmValidationError::NonPositive("cpu_count"));
        }
        if self.memory_mib == 0 {
            return Err(RealmValidationError::NonPositive("memory_mib"));
        }
        if self.disks.is_empty() {
            return Err(RealmValidationError::MissingDisk);
        }

        let mut disk_ids = HashSet::with_capacity(self.disks.len());
        let mut disk_paths = HashSet::with_capacity(self.disks.len());
        for disk in &self.disks {
            if disk.id.is_nil() {
                return Err(RealmValidationError::NilIdentifier("disk id"));
            }
            if disk.logical_size_bytes == 0 {
                return Err(RealmValidationError::NonPositive("disk logical_size_bytes"));
            }
            if !disk_ids.insert(disk.id) {
                return Err(RealmValidationError::DuplicateDiskId);
            }
            if !disk_paths.insert(disk.path.clone()) {
                return Err(RealmValidationError::DuplicateDiskPath);
            }
        }

        let RealmBackend::Vz(VzBackend::MacOs(vz)) = &self.backend;
        if self.kind != RealmKind::VirtualMachine
            || self.platform.operating_system != GuestOperatingSystem::MacOs
            || self.platform.architecture != Architecture::Aarch64
        {
            return Err(RealmValidationError::UnsupportedVzPlatform);
        }
        if vz.restore_image.version.is_empty() {
            return Err(RealmValidationError::Empty("restore image version"));
        }
        if vz.restore_image.build.is_empty() {
            return Err(RealmValidationError::Empty("restore image build"));
        }
        if vz.restore_image.minimum_cpu_count == 0 {
            return Err(RealmValidationError::NonPositive(
                "restore image minimum_cpu_count",
            ));
        }
        if vz.restore_image.minimum_memory_mib == 0 {
            return Err(RealmValidationError::NonPositive(
                "restore image minimum_memory_mib",
            ));
        }
        if self.cpu_count < vz.restore_image.minimum_cpu_count {
            return Err(RealmValidationError::BelowMinimum {
                field: "cpu_count",
                actual: self.cpu_count,
                minimum: vz.restore_image.minimum_cpu_count,
            });
        }
        if self.memory_mib < vz.restore_image.minimum_memory_mib {
            return Err(RealmValidationError::BelowMinimum {
                field: "memory_mib",
                actual: self.memory_mib,
                minimum: vz.restore_image.minimum_memory_mib,
            });
        }
        if vz.display.width_pixels == 0
            || vz.display.height_pixels == 0
            || vz.display.pixels_per_inch == 0
        {
            return Err(RealmValidationError::NonPositive("display dimension"));
        }
        if vz.hardware_model.is_empty() || vz.machine_identifier.is_empty() {
            return Err(RealmValidationError::MissingVzIdentity);
        }
        if !is_mac_address(&vz.network.mac_address) {
            return Err(RealmValidationError::InvalidMacAddress(
                vz.network.mac_address.clone(),
            ));
        }
        Ok(())
    }
}

fn is_mac_address(value: &str) -> bool {
    let mut components = value.split(':');
    let valid = components
        .by_ref()
        .take(6)
        .all(|part| part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_hexdigit()));
    valid && components.next().is_none() && value.matches(':').count() == 5
}
