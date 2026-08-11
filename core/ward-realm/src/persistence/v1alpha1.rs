// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_with::{base64::Base64, serde_as};
use uuid::Uuid;

use crate::{
    Architecture, Disk, DiskFormat, GuestOperatingSystem, MacOsRestoreImage, NetworkMode, Platform,
    Realm, RealmBackend, RealmBundleError, RealmKind, RelativePath, VzBackend, VzDisplay,
    VzInstallationState, VzMacOsConfiguration, VzNetwork,
};

pub(super) const SCHEMA_VERSION: &str = "v1alpha1";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    schema_version: String,
    id: Uuid,
    kind: String,
    backend: String,
    platform: PlatformManifest,
    cpu_count: u64,
    memory_mib: u64,
    disks: Vec<DiskManifest>,
    vz: VzManifest,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PlatformManifest {
    os: String,
    architecture: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DiskManifest {
    id: Uuid,
    path: String,
    format: String,
    logical_size_bytes: u64,
}

#[serde_as]
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VzManifest {
    installation_state: String,
    restore_image: RestoreImageManifest,
    display: DisplayManifest,
    network: NetworkManifest,
    #[serde_as(as = "Base64")]
    hardware_model: Vec<u8>,
    #[serde_as(as = "Base64")]
    machine_identifier: Vec<u8>,
    auxiliary_storage: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RestoreImageManifest {
    version: String,
    build: String,
    minimum_cpu_count: u64,
    minimum_memory_mib: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DisplayManifest {
    width_pixels: u64,
    height_pixels: u64,
    pixels_per_inch: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NetworkManifest {
    mode: String,
    mac_address: String,
}

pub(super) fn decode(value: Value) -> Result<Realm, RealmBundleError> {
    let manifest: Manifest =
        serde_json::from_value(value).map_err(RealmBundleError::InvalidJson)?;
    if manifest.schema_version != SCHEMA_VERSION {
        return Err(RealmBundleError::UnsupportedSchemaVersion(
            manifest.schema_version,
        ));
    }

    let kind = match manifest.kind.as_str() {
        "vm" => RealmKind::VirtualMachine,
        "container" => RealmKind::Container,
        other => return Err(invalid_value("kind", other)),
    };
    if manifest.backend != "vz" {
        return Err(invalid_value("backend", &manifest.backend));
    }
    let operating_system = match manifest.platform.os.as_str() {
        "macos" => GuestOperatingSystem::MacOs,
        "linux" => GuestOperatingSystem::Linux,
        "windows" => GuestOperatingSystem::Windows,
        other => return Err(invalid_value("platform.os", other)),
    };
    let architecture = match manifest.platform.architecture.as_str() {
        "aarch64" => Architecture::Aarch64,
        "x86_64" => Architecture::X86_64,
        other => return Err(invalid_value("platform.architecture", other)),
    };

    let disks = manifest
        .disks
        .into_iter()
        .map(|disk| {
            let format = match disk.format.as_str() {
                "raw" => DiskFormat::Raw,
                "qcow2" => DiskFormat::Qcow2,
                "asif" => DiskFormat::Asif,
                other => return Err(invalid_value("disks[].format", other)),
            };
            Ok(Disk {
                id: disk.id,
                path: RelativePath::new(disk.path)?,
                format,
                logical_size_bytes: disk.logical_size_bytes,
            })
        })
        .collect::<Result<Vec<_>, RealmBundleError>>()?;

    let installation_state = match manifest.vz.installation_state.as_str() {
        "prepared" => VzInstallationState::Prepared,
        "installing" => VzInstallationState::Installing,
        "installed" => VzInstallationState::Installed,
        "installation_failed" => VzInstallationState::InstallationFailed,
        other => return Err(invalid_value("vz.installation_state", other)),
    };
    if manifest.vz.network.mode != "nat" {
        return Err(invalid_value("vz.network.mode", &manifest.vz.network.mode));
    }

    let realm = Realm {
        id: manifest.id,
        kind,
        platform: Platform {
            operating_system,
            architecture,
        },
        cpu_count: manifest.cpu_count,
        memory_mib: manifest.memory_mib,
        disks,
        backend: RealmBackend::Vz(VzBackend::MacOs(VzMacOsConfiguration {
            installation_state,
            restore_image: MacOsRestoreImage {
                version: manifest.vz.restore_image.version,
                build: manifest.vz.restore_image.build,
                minimum_cpu_count: manifest.vz.restore_image.minimum_cpu_count,
                minimum_memory_mib: manifest.vz.restore_image.minimum_memory_mib,
            },
            display: VzDisplay {
                width_pixels: manifest.vz.display.width_pixels,
                height_pixels: manifest.vz.display.height_pixels,
                pixels_per_inch: manifest.vz.display.pixels_per_inch,
            },
            network: VzNetwork {
                mode: NetworkMode::Nat,
                mac_address: manifest.vz.network.mac_address,
            },
            hardware_model: manifest.vz.hardware_model,
            machine_identifier: manifest.vz.machine_identifier,
            auxiliary_storage: RelativePath::new(manifest.vz.auxiliary_storage)?,
        })),
    };
    realm.validate()?;
    Ok(realm)
}

pub(super) fn encode(realm: &Realm) -> Result<Manifest, RealmBundleError> {
    realm.validate()?;
    let RealmBackend::Vz(VzBackend::MacOs(vz)) = &realm.backend;
    let disks = realm
        .disks
        .iter()
        .map(|disk| DiskManifest {
            id: disk.id,
            path: disk.path.as_str().to_owned(),
            format: match disk.format {
                DiskFormat::Raw => "raw",
                DiskFormat::Qcow2 => "qcow2",
                DiskFormat::Asif => "asif",
            }
            .to_owned(),
            logical_size_bytes: disk.logical_size_bytes,
        })
        .collect();

    Ok(Manifest {
        schema_version: SCHEMA_VERSION.to_owned(),
        id: realm.id,
        kind: match realm.kind {
            RealmKind::VirtualMachine => "vm",
            RealmKind::Container => "container",
        }
        .to_owned(),
        backend: "vz".to_owned(),
        platform: PlatformManifest {
            os: match realm.platform.operating_system {
                GuestOperatingSystem::MacOs => "macos",
                GuestOperatingSystem::Linux => "linux",
                GuestOperatingSystem::Windows => "windows",
            }
            .to_owned(),
            architecture: match realm.platform.architecture {
                Architecture::Aarch64 => "aarch64",
                Architecture::X86_64 => "x86_64",
            }
            .to_owned(),
        },
        cpu_count: realm.cpu_count,
        memory_mib: realm.memory_mib,
        disks,
        vz: VzManifest {
            installation_state: vz.installation_state.to_string(),
            restore_image: RestoreImageManifest {
                version: vz.restore_image.version.clone(),
                build: vz.restore_image.build.clone(),
                minimum_cpu_count: vz.restore_image.minimum_cpu_count,
                minimum_memory_mib: vz.restore_image.minimum_memory_mib,
            },
            display: DisplayManifest {
                width_pixels: vz.display.width_pixels,
                height_pixels: vz.display.height_pixels,
                pixels_per_inch: vz.display.pixels_per_inch,
            },
            network: NetworkManifest {
                mode: "nat".to_owned(),
                mac_address: vz.network.mac_address.clone(),
            },
            hardware_model: vz.hardware_model.clone(),
            machine_identifier: vz.machine_identifier.clone(),
            auxiliary_storage: vz.auxiliary_storage.as_str().to_owned(),
        },
    })
}

fn invalid_value(field: &str, value: &str) -> RealmBundleError {
    RealmBundleError::InvalidManifest(format!("{field} has an unsupported value: {value}"))
}
