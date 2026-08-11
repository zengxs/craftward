// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

mod v1alpha1;

use serde_json::Value;

use crate::{Realm, RealmBundleError};

pub(super) fn decode(bytes: &[u8]) -> Result<Realm, RealmBundleError> {
    let value: Value = serde_json::from_slice(bytes).map_err(RealmBundleError::InvalidJson)?;
    match value.get("schema_version") {
        Some(Value::String(version)) if version == v1alpha1::SCHEMA_VERSION => {
            v1alpha1::decode(value)
        }
        Some(Value::String(version)) => {
            Err(RealmBundleError::UnsupportedSchemaVersion(version.clone()))
        }
        Some(other) => Err(RealmBundleError::UnsupportedSchemaVersion(
            other.to_string(),
        )),
        None => Err(RealmBundleError::UnsupportedSchemaVersion(
            "missing".to_owned(),
        )),
    }
}

pub(super) fn encode(realm: &Realm) -> Result<Vec<u8>, RealmBundleError> {
    let manifest = v1alpha1::encode(realm)?;
    serde_json::to_vec_pretty(&manifest).map_err(RealmBundleError::InvalidJson)
}
