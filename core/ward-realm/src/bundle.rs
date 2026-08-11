// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::DirBuilderExt;
use uuid::Uuid;

use crate::persistence;
use crate::{
    DiskFormat, NewVzMacOsRealm, Realm, RealmBackend, RealmBundleError, VzBackend, VzDisplay,
    VzInstallationState,
};

const MANIFEST_FILE_NAME: &str = "manifest.json";
const DISKS_DIRECTORY_NAME: &str = "disks";
const SYSTEM_DISK_RELATIVE_PATH: &str = "disks/disk0.img";
const AUXILIARY_STORAGE_RELATIVE_PATH: &str = "auxiliary_storage.bin";
const STATE_DIRECTORY_NAME: &str = "state";
const MACHINE_STATE_FILE_NAME: &str = "machine.vzvmsave";
const SAVING_MACHINE_STATE_FILE_NAME: &str = ".machine.vzvmsave.saving";
const RESTORING_MACHINE_STATE_FILE_NAME: &str = ".machine.vzvmsave.restoring";

/// A Realm bundle being populated before it is atomically published.
#[derive(Debug)]
pub struct PendingRealmBundle {
    destination: PathBuf,
    staging: PathBuf,
    published: bool,
}

impl PendingRealmBundle {
    pub fn begin(destination: impl Into<PathBuf>) -> Result<Self, RealmBundleError> {
        let destination = destination.into();
        validate_bundle_path(&destination)?;
        if destination.exists() {
            return Err(RealmBundleError::DestinationExists(destination));
        }

        let parent = destination
            .parent()
            .ok_or(RealmBundleError::InvalidBundlePath)?;
        fs::create_dir_all(parent).map_err(|error| {
            RealmBundleError::io("create bundle parent directory", parent, error)
        })?;

        let file_name = destination
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(RealmBundleError::InvalidBundlePath)?;
        let staging = parent.join(format!(".{file_name}.partial.{}", Uuid::new_v4()));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        builder.mode(0o700);
        builder
            .create(&staging)
            .map_err(|error| RealmBundleError::io("create staging directory", &staging, error))?;

        let disks = staging.join(DISKS_DIRECTORY_NAME);
        if let Err(error) = fs::create_dir(&disks) {
            let _ = fs::remove_dir_all(&staging);
            return Err(RealmBundleError::io("create disk directory", disks, error));
        }

        Ok(Self {
            destination,
            staging,
            published: false,
        })
    }

    #[must_use]
    pub fn system_disk_path(&self) -> PathBuf {
        self.staging.join(SYSTEM_DISK_RELATIVE_PATH)
    }

    #[must_use]
    pub fn auxiliary_storage_path(&self) -> PathBuf {
        self.staging.join(AUXILIARY_STORAGE_RELATIVE_PATH)
    }

    pub fn publish_vz_macos(
        mut self,
        metadata: NewVzMacOsRealm,
    ) -> Result<RealmBundle, RealmBundleError> {
        let realm = Realm::new_vz_macos(metadata)?;
        let staging = fs::canonicalize(&self.staging).map_err(|error| {
            RealmBundleError::io("resolve staging directory", &self.staging, error)
        })?;
        let bundle = RealmBundle {
            root: staging,
            realm,
        };
        bundle.validate_artifacts()?;
        bundle.persist()?;

        if self.destination.exists() {
            return Err(RealmBundleError::DestinationExists(
                self.destination.clone(),
            ));
        }
        fs::rename(&self.staging, &self.destination).map_err(|error| {
            RealmBundleError::io("publish Realm bundle", &self.destination, error)
        })?;
        self.published = true;

        let destination = fs::canonicalize(&self.destination).map_err(|error| {
            RealmBundleError::io("resolve published Realm bundle", &self.destination, error)
        })?;

        Ok(RealmBundle {
            root: destination,
            realm: bundle.realm,
        })
    }
}

impl Drop for PendingRealmBundle {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.staging);
        }
    }
}

/// A validated Realm bundle and its current in-memory domain model.
#[derive(Debug)]
pub struct RealmBundle {
    root: PathBuf,
    realm: Realm,
}

impl RealmBundle {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, RealmBundleError> {
        let root = root.into();
        validate_bundle_path(&root)?;
        let root = fs::canonicalize(&root)
            .map_err(|error| RealmBundleError::io("open Realm bundle", &root, error))?;
        if !root.is_dir() {
            return Err(RealmBundleError::InvalidBundlePath);
        }

        let manifest_path = root.join(MANIFEST_FILE_NAME);
        let resolved_manifest = fs::canonicalize(&manifest_path).map_err(|error| {
            RealmBundleError::io("resolve Realm manifest", &manifest_path, error)
        })?;
        if !resolved_manifest.starts_with(&root) {
            return Err(RealmBundleError::ArtifactEscapesBundle(manifest_path));
        }
        let bytes = fs::read(&resolved_manifest)
            .map_err(|error| RealmBundleError::io("read Realm manifest", &manifest_path, error))?;
        let realm = persistence::decode(&bytes)?;
        realm.validate()?;

        let bundle = Self { root, realm };
        bundle.validate_artifacts()?;
        Ok(bundle)
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn realm(&self) -> &Realm {
        &self.realm
    }

    pub fn require_vz_macos_installation_state(
        &self,
        expected: VzInstallationState,
    ) -> Result<(), RealmBundleError> {
        let RealmBackend::Vz(VzBackend::MacOs(vz)) = &self.realm.backend;
        if vz.installation_state != expected {
            return Err(RealmBundleError::UnexpectedInstallationState {
                expected,
                actual: vz.installation_state,
            });
        }
        Ok(())
    }

    pub fn update_vz_macos_installation_state(
        &mut self,
        state: VzInstallationState,
    ) -> Result<(), RealmBundleError> {
        let RealmBackend::Vz(VzBackend::MacOs(vz)) = &mut self.realm.backend;
        let previous = vz.installation_state;
        vz.installation_state = state;
        if let Err(error) = self.persist() {
            let RealmBackend::Vz(VzBackend::MacOs(vz)) = &mut self.realm.backend;
            vz.installation_state = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn resolve_vz_macos(&self) -> Result<ResolvedVzMacOsRealm, RealmBundleError> {
        if self.realm.kind != crate::RealmKind::VirtualMachine
            || self.realm.platform.operating_system != crate::GuestOperatingSystem::MacOs
            || self.realm.platform.architecture != crate::Architecture::Aarch64
        {
            return Err(RealmBundleError::NotVzMacOs);
        }
        let RealmBackend::Vz(VzBackend::MacOs(vz)) = &self.realm.backend;

        let disks = self
            .realm
            .disks
            .iter()
            .map(|disk| {
                Ok(ResolvedDisk {
                    path: self.resolve_existing_file(disk.path.as_path())?,
                    format: disk.format,
                    logical_size_bytes: disk.logical_size_bytes,
                })
            })
            .collect::<Result<Vec<_>, RealmBundleError>>()?;
        let auxiliary_storage = self.resolve_existing_file(vz.auxiliary_storage.as_path())?;
        let saved_state = self.resolve_saved_state_files()?;

        Ok(ResolvedVzMacOsRealm {
            installation_state: vz.installation_state,
            cpu_count: self.realm.cpu_count,
            memory_mib: self.realm.memory_mib,
            disks,
            auxiliary_storage,
            hardware_model: vz.hardware_model.clone(),
            machine_identifier: vz.machine_identifier.clone(),
            display: vz.display,
            mac_address: vz.network.mac_address.clone(),
            saved_state,
        })
    }

    fn validate_artifacts(&self) -> Result<(), RealmBundleError> {
        let RealmBackend::Vz(VzBackend::MacOs(vz)) = &self.realm.backend;
        for disk in &self.realm.disks {
            self.resolve_existing_file(disk.path.as_path())?;
        }
        self.resolve_existing_file(vz.auxiliary_storage.as_path())?;
        Ok(())
    }

    fn resolve_existing_file(&self, relative: &Path) -> Result<PathBuf, RealmBundleError> {
        let path = self.root.join(relative);
        let resolved = fs::canonicalize(&path)
            .map_err(|error| RealmBundleError::io("resolve Realm artifact", &path, error))?;
        if !resolved.starts_with(&self.root) {
            return Err(RealmBundleError::ArtifactEscapesBundle(path));
        }
        if !resolved.is_file() {
            return Err(RealmBundleError::ArtifactNotFile(path));
        }
        Ok(resolved)
    }

    fn resolve_saved_state_files(&self) -> Result<ResolvedSavedStateFiles, RealmBundleError> {
        let state = self.root.join(STATE_DIRECTORY_NAME);
        self.validate_optional_path(&state)?;
        let files = ResolvedSavedStateFiles {
            machine_state: state.join(MACHINE_STATE_FILE_NAME),
            saving: state.join(SAVING_MACHINE_STATE_FILE_NAME),
            restoring: state.join(RESTORING_MACHINE_STATE_FILE_NAME),
        };
        for path in [&files.machine_state, &files.saving, &files.restoring] {
            self.validate_optional_file(path)?;
        }
        Ok(files)
    }

    fn validate_optional_file(&self, path: &Path) -> Result<bool, RealmBundleError> {
        let Some(resolved) = self.resolve_optional_path(path)? else {
            return Ok(false);
        };
        if !resolved.is_file() {
            return Err(RealmBundleError::ArtifactNotFile(path.to_owned()));
        }
        Ok(true)
    }

    fn validate_optional_path(&self, path: &Path) -> Result<bool, RealmBundleError> {
        Ok(self.resolve_optional_path(path)?.is_some())
    }

    fn resolve_optional_path(&self, path: &Path) -> Result<Option<PathBuf>, RealmBundleError> {
        match fs::symlink_metadata(path) {
            Ok(_) => {
                let resolved = fs::canonicalize(path).map_err(|error| {
                    RealmBundleError::io("resolve optional Realm artifact", path, error)
                })?;
                if !resolved.starts_with(&self.root) {
                    return Err(RealmBundleError::ArtifactEscapesBundle(path.to_owned()));
                }
                Ok(Some(resolved))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(RealmBundleError::io(
                "inspect optional Realm artifact",
                path,
                error,
            )),
        }
    }

    fn persist(&self) -> Result<(), RealmBundleError> {
        let bytes = persistence::encode(&self.realm)?;
        let destination = self.root.join(MANIFEST_FILE_NAME);
        let temporary = self
            .root
            .join(format!(".{MANIFEST_FILE_NAME}.{}.tmp", Uuid::new_v4()));

        let result = write_manifest_file(&temporary, &bytes)
            .and_then(|()| fs::rename(&temporary, &destination));
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(RealmBundleError::io(
                "write Realm manifest",
                destination,
                error,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedDisk {
    pub path: PathBuf,
    pub format: DiskFormat,
    pub logical_size_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSavedStateFiles {
    pub machine_state: PathBuf,
    pub saving: PathBuf,
    pub restoring: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedVzMacOsRealm {
    pub installation_state: VzInstallationState,
    pub cpu_count: u64,
    pub memory_mib: u64,
    pub disks: Vec<ResolvedDisk>,
    pub auxiliary_storage: PathBuf,
    pub hardware_model: Vec<u8>,
    pub machine_identifier: Vec<u8>,
    pub display: VzDisplay,
    pub mac_address: String,
    pub saved_state: ResolvedSavedStateFiles,
}

fn validate_bundle_path(path: &Path) -> Result<(), RealmBundleError> {
    if !path.is_absolute() || path.as_os_str().is_empty() || path.file_name().is_none() {
        return Err(RealmBundleError::InvalidBundlePath);
    }
    Ok(())
}

fn write_manifest_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()
}
