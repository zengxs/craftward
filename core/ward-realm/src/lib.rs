// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

//! Persistent domain model for isolated execution realms.
//!
//! This crate owns Realm bundle layout, validation, and schema migration. A
//! backend adapter receives resolved, typed values and never needs to parse a
//! bundle manifest or infer an artifact path.

mod bundle;
mod error;
mod model;
mod persistence;

pub use bundle::{
    PendingRealmBundle, RealmBundle, ResolvedDisk, ResolvedSavedStateFiles, ResolvedVzMacOsRealm,
};
pub use error::{RealmBundleError, RealmValidationError};
pub use model::{
    Architecture, Disk, DiskFormat, GuestOperatingSystem, MIB, MacOsRestoreImage, NetworkMode,
    NewVzMacOsRealm, Platform, Realm, RealmBackend, RealmKind, RelativePath, VzBackend, VzDisplay,
    VzInstallationState, VzMacOsConfiguration, VzNetwork,
};
