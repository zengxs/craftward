// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io;
use std::path::PathBuf;

use thiserror::Error;

use crate::VzInstallationState;

/// A violation of the current Realm domain model.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum RealmValidationError {
    #[error("{0} must be greater than zero")]
    NonPositive(&'static str),
    #[error("{0} must not be empty")]
    Empty(&'static str),
    #[error("{field} ({actual}) is below its required minimum ({minimum})")]
    BelowMinimum {
        field: &'static str,
        actual: u64,
        minimum: u64,
    },
    #[error("{0} must not be a nil UUID")]
    NilIdentifier(&'static str),
    #[error("the Realm must contain at least one disk")]
    MissingDisk,
    #[error("the Realm contains duplicate disk identifiers")]
    DuplicateDiskId,
    #[error("the Realm contains duplicate disk paths")]
    DuplicateDiskPath,
    #[error("the relative path is empty, absolute, or contains a non-normal component: {0}")]
    InvalidRelativePath(String),
    #[error("the VZ backend currently supports only a macOS aarch64 virtual machine Realm")]
    UnsupportedVzPlatform,
    #[error("the VZ network MAC address is invalid: {0}")]
    InvalidMacAddress(String),
    #[error("the VZ hardware model or machine identifier is empty")]
    MissingVzIdentity,
}

/// An error while creating, opening, migrating, or updating a Realm bundle.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RealmBundleError {
    #[error("the Realm bundle path must be absolute and name a directory")]
    InvalidBundlePath,
    #[error("the Realm bundle destination already exists: {0}")]
    DestinationExists(PathBuf),
    #[error("failed to {operation} {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("the Realm manifest is not valid JSON: {0}")]
    InvalidJson(#[source] serde_json::Error),
    #[error("the Realm manifest schema version is missing or unsupported: {0}")]
    UnsupportedSchemaVersion(String),
    #[error("the Realm manifest is invalid: {0}")]
    InvalidManifest(String),
    #[error(transparent)]
    InvalidRealm(#[from] RealmValidationError),
    #[error("a Realm artifact escapes its bundle: {0}")]
    ArtifactEscapesBundle(PathBuf),
    #[error("a Realm artifact is not a regular file: {0}")]
    ArtifactNotFile(PathBuf),
    #[error("the Realm is not backed by a VZ macOS virtual machine")]
    NotVzMacOs,
    #[error("the Realm must be {expected} before this operation, but it is {actual}")]
    UnexpectedInstallationState {
        expected: VzInstallationState,
        actual: VzInstallationState,
    },
}

impl RealmBundleError {
    pub(crate) fn io(operation: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            source,
        }
    }
}
