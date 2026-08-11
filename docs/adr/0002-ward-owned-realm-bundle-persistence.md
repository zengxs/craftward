---
status: accepted
---

# Ward-Owned Realm Bundle Persistence

Realm bundles must represent virtual machines and containers without making their
persistent format the property of a particular backend. The `ward-realm` crate will
therefore own the bundle layout, manifest schema, validation, path resolution, and
migration, while backend adapters such as `ward-realm-vz` expose typed operations
and neither parse manifests nor infer bundle paths.

The machine-owned manifest will use `snake_case` JSON keys and one exact, top-level
`schema_version`. Its first version is `v1alpha1`. Backend-independent Realm data
appears once at the root, while backend-specific data is grouped under a
backend-named object; backend objects do not have independent schema versions while
Ward owns their representation. `ward-realm` decodes each supported persisted
version into the current Realm domain model and always writes the current persisted
version.

## Compatibility Policy

After `v1alpha1`, Ward automatically migrates an older supported manifest when a
migration path is still implemented, but does not promise a fixed compatibility
window before the format is declared stable. A migration must not publish its new
manifest until the resulting bundle validates. An unsupported version or failed
migration leaves the previous manifest authoritative and reports an error instead
of guessing. Opening and upgrading a bundle with a newer Ward release may make it
unreadable by an older release.

Manifest compatibility does not imply backend saved-state compatibility. If a
backend can no longer safely restore saved execution state with the current machine
configuration, Ward preserves the Realm's durable storage and requires the saved
state to be discarded before a cold start rather than attempting an unsafe restore.

## Considered Options

- Backend-owned manifests were rejected because they duplicate shared persistence
  concerns and make the first backend's assumptions part of the Realm abstraction.
- TOML was rejected because the manifest is machine-owned and automatically
  migrated; JSON provides the required structured representation without implying
  that users should edit it.
- Independent versions inside backend objects were deferred because there is only
  one owner and migration boundary. They become useful only if an independently
  versioned provider owns an opaque payload.
- Permanent backward compatibility was rejected while the format is experimental
  because it would constrain the domain model before multiple backends validate it.

## Consequences

`ward-realm` becomes the only manifest owner. Backend adapters receive resolved,
typed values and contain no manifest-schema or bundle-layout logic.
Persisted-version types remain private mapping representations rather than becoming
the public Realm model, allowing the current domain model to evolve without changing
the on-disk schema for every internal refactoring.
