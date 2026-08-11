// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use uuid::Uuid;
use ward_realm::{
    MacOsRestoreImage, NewVzMacOsRealm, PendingRealmBundle, RealmBundle, RealmBundleError,
    RealmValidationError, VzDisplay, VzInstallationState,
};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("ward-realm-test-{}", Uuid::new_v4()));
        fs::create_dir(&path).expect("the test directory should be created");
        Self(path)
    }

    fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.0.join(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn prepared_metadata() -> NewVzMacOsRealm {
    NewVzMacOsRealm {
        restore_image: MacOsRestoreImage {
            version: "15.6.1".to_owned(),
            build: "24G90".to_owned(),
            minimum_cpu_count: 2,
            minimum_memory_mib: 4096,
        },
        disk_logical_size_bytes: 64 * 1024 * 1024 * 1024,
        display: VzDisplay {
            width_pixels: 1920,
            height_pixels: 1200,
            pixels_per_inch: 80,
        },
        mac_address: "02:00:00:00:00:01".to_owned(),
        hardware_model: vec![1, 2, 3],
        machine_identifier: vec![4, 5, 6],
    }
}

fn current_manifest(disk_path: &str) -> Value {
    json!({
        "schema_version": "v1alpha1",
        "id": Uuid::new_v4(),
        "kind": "vm",
        "backend": "vz",
        "platform": { "os": "macos", "architecture": "aarch64" },
        "cpu_count": 2,
        "memory_mib": 4096,
        "disks": [{
            "id": Uuid::new_v4(),
            "path": disk_path,
            "format": "raw",
            "logical_size_bytes": 1024
        }],
        "vz": {
            "installation_state": "installed",
            "restore_image": {
                "version": "15.6.1",
                "build": "24G90",
                "minimum_cpu_count": 2,
                "minimum_memory_mib": 4096
            },
            "display": {
                "width_pixels": 1920,
                "height_pixels": 1200,
                "pixels_per_inch": 80
            },
            "network": { "mode": "nat", "mac_address": "02:00:00:00:00:01" },
            "hardware_model": "AQID",
            "machine_identifier": "BAUG",
            "auxiliary_storage": "auxiliary_storage.bin"
        }
    })
}

#[test]
fn publishes_and_reopens_a_v1alpha1_bundle() {
    let directory = TestDirectory::new();
    let destination = directory.join("Example.realm");
    let pending = PendingRealmBundle::begin(&destination).expect("preparation should start");
    fs::write(pending.system_disk_path(), b"disk").expect("the disk should be created");
    fs::write(pending.auxiliary_storage_path(), b"aux")
        .expect("the auxiliary storage should be created");

    let mut bundle = pending
        .publish_vz_macos(prepared_metadata())
        .expect("the bundle should be published");
    let manifest: Value = serde_json::from_slice(
        &fs::read(destination.join("manifest.json")).expect("the manifest should exist"),
    )
    .expect("the manifest should be JSON");
    assert_eq!(manifest["schema_version"], "v1alpha1");
    assert_eq!(manifest["kind"], "vm");
    assert_eq!(manifest["backend"], "vz");
    assert_eq!(manifest["memory_mib"], 4096);
    assert_eq!(manifest["vz"]["hardware_model"], "AQID");

    bundle
        .update_vz_macos_installation_state(VzInstallationState::Installed)
        .expect("the installation state should persist");
    let reopened = RealmBundle::open(&destination).expect("the bundle should reopen");
    reopened
        .require_vz_macos_installation_state(VzInstallationState::Installed)
        .expect("the installed state should survive reopening");
    let resolved = reopened
        .resolve_vz_macos()
        .expect("the VZ configuration should resolve");
    let destination = fs::canonicalize(destination).expect("the destination should resolve");
    assert_eq!(resolved.disks.len(), 1);
    assert_eq!(resolved.disks[0].path, destination.join("disks/disk0.img"));
    assert_eq!(
        resolved.auxiliary_storage,
        destination.join("auxiliary_storage.bin")
    );
    assert_eq!(
        resolved.saved_state.machine_state,
        destination.join("state/machine.vzvmsave")
    );
}

#[test]
fn rejects_artifact_paths_that_escape_the_bundle() {
    let directory = TestDirectory::new();
    let bundle_path = directory.join("Invalid.realm");
    fs::create_dir(&bundle_path).expect("the bundle should be created");
    fs::write(bundle_path.join("auxiliary_storage.bin"), b"aux")
        .expect("the auxiliary storage should be created");
    let manifest = current_manifest("../outside.img");
    fs::write(
        bundle_path.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("the manifest should encode"),
    )
    .expect("the manifest should be created");

    let error = RealmBundle::open(&bundle_path).expect_err("the escaping path should fail");
    assert!(matches!(error, RealmBundleError::InvalidRealm(_)));
}

#[cfg(unix)]
#[test]
fn rejects_symlinks_that_escape_the_bundle() {
    use std::os::unix::fs::symlink;

    let directory = TestDirectory::new();
    let outside_disk = directory.join("outside.img");
    fs::write(&outside_disk, b"outside").expect("the outside disk should be created");
    let bundle_path = directory.join("Symlink.realm");
    fs::create_dir_all(bundle_path.join("disks")).expect("the disk directory should be created");
    symlink(&outside_disk, bundle_path.join("disks/disk0.img"))
        .expect("the disk symlink should be created");
    fs::write(bundle_path.join("auxiliary_storage.bin"), b"aux")
        .expect("the auxiliary storage should be created");
    fs::write(
        bundle_path.join("manifest.json"),
        serde_json::to_vec_pretty(&current_manifest("disks/disk0.img"))
            .expect("the manifest should encode"),
    )
    .expect("the manifest should be created");

    let error = RealmBundle::open(&bundle_path).expect_err("the escaping symlink should fail");
    assert!(matches!(error, RealmBundleError::ArtifactEscapesBundle(_)));
}

#[test]
fn rejects_a_disk_path_that_is_not_a_file() {
    let directory = TestDirectory::new();
    let bundle_path = directory.join("DiskDirectory.realm");
    fs::create_dir_all(bundle_path.join("disks/disk0.img"))
        .expect("the disk directory should be created");
    fs::write(bundle_path.join("auxiliary_storage.bin"), b"aux")
        .expect("the auxiliary storage should be created");
    fs::write(
        bundle_path.join("manifest.json"),
        serde_json::to_vec_pretty(&current_manifest("disks/disk0.img"))
            .expect("the manifest should encode"),
    )
    .expect("the manifest should be created");

    let error = RealmBundle::open(&bundle_path).expect_err("a directory should not be a disk");
    assert!(matches!(error, RealmBundleError::ArtifactNotFile(_)));
}

#[test]
fn rejects_an_auxiliary_storage_path_that_is_not_a_file() {
    let directory = TestDirectory::new();
    let bundle_path = directory.join("AuxiliaryStorageDirectory.realm");
    fs::create_dir_all(bundle_path.join("disks")).expect("the disk directory should be created");
    fs::write(bundle_path.join("disks/disk0.img"), b"disk").expect("the disk should be created");
    fs::create_dir(bundle_path.join("auxiliary_storage.bin"))
        .expect("the auxiliary storage directory should be created");
    fs::write(
        bundle_path.join("manifest.json"),
        serde_json::to_vec_pretty(&current_manifest("disks/disk0.img"))
            .expect("the manifest should encode"),
    )
    .expect("the manifest should be created");

    let error =
        RealmBundle::open(&bundle_path).expect_err("a directory should not be auxiliary storage");
    assert!(matches!(error, RealmBundleError::ArtifactNotFile(_)));
}

#[cfg(unix)]
#[test]
fn rejects_saved_state_symlinks_that_escape_the_bundle() {
    use std::os::unix::fs::symlink;

    let directory = TestDirectory::new();
    let destination = directory.join("StateSymlink.realm");
    let pending = PendingRealmBundle::begin(&destination).expect("preparation should start");
    fs::write(pending.system_disk_path(), b"disk").expect("the disk should be created");
    fs::write(pending.auxiliary_storage_path(), b"aux")
        .expect("the auxiliary storage should be created");
    let bundle = pending
        .publish_vz_macos(prepared_metadata())
        .expect("the bundle should be published");

    let outside_state = directory.join("outside.vzvmsave");
    fs::write(&outside_state, b"state").expect("the outside state should be created");
    fs::create_dir(destination.join("state")).expect("the state directory should be created");
    symlink(&outside_state, destination.join("state/machine.vzvmsave"))
        .expect("the state symlink should be created");

    let error = bundle
        .resolve_vz_macos()
        .expect_err("the escaping state symlink should fail");
    assert!(matches!(error, RealmBundleError::ArtifactEscapesBundle(_)));
}

#[test]
fn rejects_a_saved_state_path_that_is_not_a_file() {
    let directory = TestDirectory::new();
    let destination = directory.join("StateDirectory.realm");
    let pending = PendingRealmBundle::begin(&destination).expect("preparation should start");
    fs::write(pending.system_disk_path(), b"disk").expect("the disk should be created");
    fs::write(pending.auxiliary_storage_path(), b"aux")
        .expect("the auxiliary storage should be created");
    let bundle = pending
        .publish_vz_macos(prepared_metadata())
        .expect("the bundle should be published");

    fs::create_dir_all(destination.join("state/machine.vzvmsave"))
        .expect("the saved-state directory should be created");

    let error = bundle
        .resolve_vz_macos()
        .expect_err("a directory should not be a saved-state file");
    assert!(matches!(error, RealmBundleError::ArtifactNotFile(_)));
}

#[test]
fn leaves_an_unsupported_manifest_unchanged() {
    let directory = TestDirectory::new();
    let bundle_path = directory.join("Future.realm");
    fs::create_dir(&bundle_path).expect("the bundle should be created");
    let manifest_path = bundle_path.join("manifest.json");
    let original = b"{\n  \"schema_version\": \"v1alpha2\"\n}\n";
    fs::write(&manifest_path, original).expect("the manifest should be created");

    let error = RealmBundle::open(&bundle_path).expect_err("the version should be unsupported");
    assert!(matches!(
        error,
        RealmBundleError::UnsupportedSchemaVersion(version) if version == "v1alpha2"
    ));
    assert_eq!(
        fs::read(manifest_path).expect("the manifest should remain readable"),
        original
    );
}

#[test]
fn rejects_a_manifest_without_schema_version_without_modifying_it() {
    let directory = TestDirectory::new();
    let bundle_path = directory.join("MissingSchemaVersion.realm");
    fs::create_dir(&bundle_path).expect("the bundle should be created");
    let manifest_path = bundle_path.join("manifest.json");
    let original = b"{}\n";
    fs::write(&manifest_path, original).expect("the manifest should be created");

    let error =
        RealmBundle::open(&bundle_path).expect_err("the missing schema version should fail");
    assert!(matches!(
        error,
        RealmBundleError::UnsupportedSchemaVersion(version) if version == "missing"
    ));
    assert_eq!(
        fs::read(manifest_path).expect("the manifest should remain readable"),
        original
    );
}

#[test]
fn rejects_cpu_count_below_the_restore_image_minimum() {
    let directory = TestDirectory::new();
    let bundle_path = directory.join("InsufficientCpu.realm");
    fs::create_dir(&bundle_path).expect("the bundle should be created");
    let mut manifest = current_manifest("disks/disk0.img");
    manifest["cpu_count"] = json!(1);
    fs::write(
        bundle_path.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("the manifest should encode"),
    )
    .expect("the manifest should be created");

    let error = RealmBundle::open(&bundle_path).expect_err("the CPU count should fail");
    assert!(matches!(
        error,
        RealmBundleError::InvalidRealm(RealmValidationError::BelowMinimum {
            field: "cpu_count",
            actual: 1,
            minimum: 2,
        })
    ));
}

#[test]
fn rejects_memory_below_the_restore_image_minimum() {
    let directory = TestDirectory::new();
    let bundle_path = directory.join("InsufficientMemory.realm");
    fs::create_dir(&bundle_path).expect("the bundle should be created");
    let mut manifest = current_manifest("disks/disk0.img");
    manifest["memory_mib"] = json!(2048);
    fs::write(
        bundle_path.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("the manifest should encode"),
    )
    .expect("the manifest should be created");

    let error = RealmBundle::open(&bundle_path).expect_err("the memory size should fail");
    assert!(matches!(
        error,
        RealmBundleError::InvalidRealm(RealmValidationError::BelowMinimum {
            field: "memory_mib",
            actual: 2048,
            minimum: 4096,
        })
    ));
}
