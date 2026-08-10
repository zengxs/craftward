// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
mod ffi;

/// The default logical size of a new macOS system disk.
///
/// The disk is sparse, so this logical capacity is not allocated eagerly on
/// the host filesystem.
pub const DEFAULT_MACOS_DISK_SIZE: u64 = 64 * 1024 * 1024 * 1024;

/// Parameters for preparing the persistent files needed by a macOS guest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacOsBundleRequest {
    /// An absolute path to a local macOS IPSW restore image.
    pub restore_image: PathBuf,
    /// An absolute path for the new bundle directory.
    pub destination: PathBuf,
    /// The logical size of the sparse RAW system disk, in bytes.
    pub disk_size: u64,
}

impl MacOsBundleRequest {
    /// Creates a request using [`DEFAULT_MACOS_DISK_SIZE`].
    pub fn new(restore_image: impl Into<PathBuf>, destination: impl Into<PathBuf>) -> Self {
        Self {
            restore_image: restore_image.into(),
            destination: destination.into(),
            disk_size: DEFAULT_MACOS_DISK_SIZE,
        }
    }

    /// Sets the logical system disk size, in bytes.
    #[must_use]
    pub const fn with_disk_size(mut self, disk_size: u64) -> Self {
        self.disk_size = disk_size;
        self
    }
}

/// A semantic macOS version stored in a realm bundle manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MacOsVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

/// Details of a prepared macOS realm bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacOsBundleInfo {
    pub path: PathBuf,
    pub build_version: String,
    pub operating_system_version: MacOsVersion,
    pub minimum_cpu_count: u64,
    pub minimum_memory_size: u64,
    pub disk_size: u64,
}

/// Parameters for installing macOS into a prepared realm bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacOsBundleInstallationRequest {
    /// An absolute path to the local IPSW used to prepare the bundle.
    pub restore_image: PathBuf,
    /// An absolute path to a prepared macOS realm bundle.
    pub bundle: PathBuf,
}

impl MacOsBundleInstallationRequest {
    /// Creates an installation request for a prepared bundle.
    pub fn new(restore_image: impl Into<PathBuf>, bundle: impl Into<PathBuf>) -> Self {
        Self {
            restore_image: restore_image.into(),
            bundle: bundle.into(),
        }
    }
}

/// An error encountered while starting or completing bundle preparation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MacOsBundlePreparationError {
    /// The current host cannot create Virtualization.framework macOS guests.
    UnsupportedHost,
    /// A path was empty, relative, did not name a bundle, or contained a NUL
    /// byte.
    InvalidPath(&'static str),
    /// The requested logical disk size is zero, unaligned, or cannot be
    /// represented by the host.
    InvalidDiskSize(u64),
    /// Virtualization.framework or the host filesystem rejected the request.
    Native {
        domain: String,
        code: i64,
        message: String,
    },
}

impl fmt::Display for MacOsBundlePreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedHost => formatter
                .write_str("this host cannot create a Virtualization.framework macOS guest"),
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

impl std::error::Error for MacOsBundlePreparationError {}

/// An error encountered while starting or completing macOS installation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MacOsBundleInstallationError {
    /// The current host cannot install Virtualization.framework macOS guests.
    UnsupportedHost,
    /// A path was empty, relative, did not name a bundle, or contained a NUL
    /// byte.
    InvalidPath(&'static str),
    /// The bundle, virtual machine configuration, or installer was rejected.
    Native {
        domain: String,
        code: i64,
        message: String,
    },
}

impl fmt::Display for MacOsBundleInstallationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedHost => formatter
                .write_str("this host cannot install a Virtualization.framework macOS guest"),
            Self::InvalidPath(name) => write!(formatter, "the {name} path is invalid"),
            Self::Native {
                domain,
                code,
                message,
            } => write!(formatter, "{message} ({domain}, code {code})"),
        }
    }
}

impl std::error::Error for MacOsBundleInstallationError {}

#[cfg(target_os = "macos")]
fn path_to_c_string(
    path: &std::path::Path,
    name: &'static str,
    must_name_file: bool,
) -> Result<std::ffi::CString, &'static str> {
    use std::os::unix::ffi::OsStrExt;

    if !path.is_absolute()
        || path.as_os_str().is_empty()
        || (must_name_file && path.file_name().is_none())
    {
        return Err(name);
    }

    std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|_| name)
}

#[cfg(target_os = "macos")]
unsafe fn copy_bridge_string(value: *const std::ffi::c_char) -> String {
    if value.is_null() {
        return String::new();
    }

    // SAFETY: Bridge strings remain alive for the duration of their callback
    // and are terminated with a NUL byte.
    unsafe { std::ffi::CStr::from_ptr(value) }
        .to_string_lossy()
        .into_owned()
}

/// Returns whether the current host supports Virtualization.framework virtual
/// machines.
#[cfg(target_os = "macos")]
pub fn is_supported() -> bool {
    // SAFETY: The bridge function accepts no arguments and returns a
    // C-compatible boolean value.
    unsafe { ffi::ward_vz_is_supported() }
}

/// Returns whether the current host supports Virtualization.framework virtual
/// machines.
#[cfg(not(target_os = "macos"))]
pub const fn is_supported() -> bool {
    false
}

/// Prepares the persistent files needed to install a macOS guest from a local
/// IPSW restore image.
///
/// The destination is published atomically after all bundle files have been
/// created. The completion callback runs exactly once on an arbitrary thread,
/// but only if this function returns `Ok(())`. The callback may run before this
/// function returns.
#[cfg(target_os = "macos")]
pub fn prepare_macos_bundle(
    request: MacOsBundleRequest,
    completion: impl FnOnce(Result<MacOsBundleInfo, MacOsBundlePreparationError>) + Send + 'static,
) -> Result<(), MacOsBundlePreparationError> {
    use std::ffi::c_void;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    type Completion =
        Box<dyn FnOnce(Result<MacOsBundleInfo, MacOsBundlePreparationError>) + Send + 'static>;

    struct CompletionContext {
        destination: PathBuf,
        completion: Option<Completion>,
    }

    unsafe extern "C" fn bundle_prepared(
        context: *mut c_void,
        bundle_info: *const ffi::WardVzMacOsBundleInfo,
        error: *const ffi::WardVzError,
    ) {
        if context.is_null() {
            return;
        }

        // SAFETY: This pointer was produced by Box::into_raw below, and the
        // bridge guarantees exactly one completion call.
        let mut context = unsafe { Box::from_raw(context.cast::<CompletionContext>()) };
        let result = if !error.is_null() {
            // SAFETY: A non-null bridge error points to a value whose fields
            // remain valid for the duration of this completion call.
            let error = unsafe { &*error };
            Err(MacOsBundlePreparationError::Native {
                // SAFETY: See the bridge error lifetime guarantee above.
                domain: unsafe { copy_bridge_string(error.domain) },
                code: error.code,
                // SAFETY: See the bridge error lifetime guarantee above.
                message: unsafe { copy_bridge_string(error.message) },
            })
        } else if bundle_info.is_null() {
            Err(MacOsBundlePreparationError::Native {
                domain: "app.craftward.ward-realm-vz.bridge".into(),
                code: -1,
                message: "the native bridge returned no result".into(),
            })
        } else {
            // SAFETY: A non-null bundle info pointer remains valid for the
            // duration of this completion call.
            let bundle_info = unsafe { &*bundle_info };
            Ok(MacOsBundleInfo {
                path: context.destination.clone(),
                // SAFETY: See the bundle info lifetime guarantee above.
                build_version: unsafe { copy_bridge_string(bundle_info.build_version) },
                operating_system_version: MacOsVersion {
                    major: bundle_info.os_version_major,
                    minor: bundle_info.os_version_minor,
                    patch: bundle_info.os_version_patch,
                },
                minimum_cpu_count: bundle_info.minimum_cpu_count,
                minimum_memory_size: bundle_info.minimum_memory_size,
                disk_size: bundle_info.disk_size,
            })
        };

        if let Some(completion) = context.completion.take() {
            let _ = catch_unwind(AssertUnwindSafe(|| completion(result)));
        }
    }

    if request.disk_size == 0
        || !request.disk_size.is_multiple_of(512)
        || request.disk_size > i64::MAX as u64
    {
        return Err(MacOsBundlePreparationError::InvalidDiskSize(
            request.disk_size,
        ));
    }
    let restore_image = path_to_c_string(&request.restore_image, "restore image", false)
        .map_err(MacOsBundlePreparationError::InvalidPath)?;
    let destination = path_to_c_string(&request.destination, "destination", true)
        .map_err(MacOsBundlePreparationError::InvalidPath)?;
    if !is_supported() {
        return Err(MacOsBundlePreparationError::UnsupportedHost);
    }

    let context = Box::new(CompletionContext {
        destination: request.destination,
        completion: Some(Box::new(completion)),
    });

    // SAFETY: The path pointers remain valid for the duration of this call.
    // The bridge copies them before returning and takes ownership of the
    // context pointer until it invokes bundle_prepared exactly once.
    unsafe {
        ffi::ward_vz_prepare_macos_bundle(
            restore_image.as_ptr(),
            destination.as_ptr(),
            request.disk_size,
            bundle_prepared,
            Box::into_raw(context).cast(),
        );
    }

    Ok(())
}

/// Returns [`MacOsBundlePreparationError::UnsupportedHost`] on non-macOS hosts.
#[cfg(not(target_os = "macos"))]
pub fn prepare_macos_bundle(
    _request: MacOsBundleRequest,
    _completion: impl FnOnce(Result<MacOsBundleInfo, MacOsBundlePreparationError>) + Send + 'static,
) -> Result<(), MacOsBundlePreparationError> {
    Err(MacOsBundlePreparationError::UnsupportedHost)
}

/// Installs macOS into a prepared realm bundle.
///
/// The adapter reconstructs and validates the virtual machine configuration
/// from the bundle manifest. Progress values are clamped to the inclusive
/// range `0.0..=1.0`. Callbacks may run on an arbitrary thread and may run
/// before this function returns, but are serialized with respect to each other.
/// The completion callback runs exactly once, and no progress callback runs
/// after it, but only if this function returns `Ok(())`.
#[cfg(target_os = "macos")]
pub fn install_macos_bundle(
    request: MacOsBundleInstallationRequest,
    progress: impl FnMut(f64) + Send + 'static,
    completion: impl FnOnce(Result<(), MacOsBundleInstallationError>) + Send + 'static,
) -> Result<(), MacOsBundleInstallationError> {
    use std::ffi::c_void;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::Mutex;

    type Progress = Box<dyn FnMut(f64) + Send + 'static>;
    type Completion = Box<dyn FnOnce(Result<(), MacOsBundleInstallationError>) + Send + 'static>;

    struct InstallationContext {
        progress: Mutex<Progress>,
        completion: Option<Completion>,
    }

    unsafe extern "C" fn installation_progress(context: *mut c_void, fraction_completed: f64) {
        if context.is_null() {
            return;
        }

        // SAFETY: The bridge owns the context until its completion call. It
        // invokes progress serially and never after completion.
        let context = unsafe { &*context.cast::<InstallationContext>() };
        let Ok(mut progress) = context.progress.lock() else {
            return;
        };
        let fraction_completed = fraction_completed.clamp(0.0, 1.0);
        let _ = catch_unwind(AssertUnwindSafe(|| progress(fraction_completed)));
    }

    unsafe extern "C" fn installation_completed(
        context: *mut c_void,
        error: *const ffi::WardVzError,
    ) {
        if context.is_null() {
            return;
        }

        // SAFETY: This pointer was produced by Box::into_raw below, and the
        // bridge guarantees exactly one completion call after progress ends.
        let mut context = unsafe { Box::from_raw(context.cast::<InstallationContext>()) };
        let result = if error.is_null() {
            Ok(())
        } else {
            // SAFETY: A non-null bridge error remains valid for the duration
            // of this completion call.
            let error = unsafe { &*error };
            Err(MacOsBundleInstallationError::Native {
                // SAFETY: See the bridge error lifetime guarantee above.
                domain: unsafe { copy_bridge_string(error.domain) },
                code: error.code,
                // SAFETY: See the bridge error lifetime guarantee above.
                message: unsafe { copy_bridge_string(error.message) },
            })
        };

        if let Some(completion) = context.completion.take() {
            let _ = catch_unwind(AssertUnwindSafe(|| completion(result)));
        }
    }

    let restore_image = path_to_c_string(&request.restore_image, "restore image", false)
        .map_err(MacOsBundleInstallationError::InvalidPath)?;
    let bundle = path_to_c_string(&request.bundle, "bundle", true)
        .map_err(MacOsBundleInstallationError::InvalidPath)?;
    if !is_supported() {
        return Err(MacOsBundleInstallationError::UnsupportedHost);
    }

    let context = Box::new(InstallationContext {
        progress: Mutex::new(Box::new(progress)),
        completion: Some(Box::new(completion)),
    });

    // SAFETY: The bridge copies both paths before returning and owns the
    // context pointer until it invokes installation_completed exactly once.
    unsafe {
        ffi::ward_vz_install_macos_bundle(
            restore_image.as_ptr(),
            bundle.as_ptr(),
            installation_progress,
            installation_completed,
            Box::into_raw(context).cast(),
        );
    }

    Ok(())
}

/// Returns [`MacOsBundleInstallationError::UnsupportedHost`] on non-macOS
/// hosts.
#[cfg(not(target_os = "macos"))]
pub fn install_macos_bundle(
    _request: MacOsBundleInstallationRequest,
    _progress: impl FnMut(f64) + Send + 'static,
    _completion: impl FnOnce(Result<(), MacOsBundleInstallationError>) + Send + 'static,
) -> Result<(), MacOsBundleInstallationError> {
    Err(MacOsBundleInstallationError::UnsupportedHost)
}
