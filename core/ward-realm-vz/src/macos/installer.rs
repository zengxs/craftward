// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt;
use std::path::PathBuf;

use super::MacOsVirtualMachineConfiguration;
#[cfg(target_os = "macos")]
use super::{
    NativeMacOsVirtualMachineConfiguration, copy_bridge_string, is_supported, path_to_c_string,
};
#[cfg(target_os = "macos")]
use crate::ffi;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacOsInstallationRequest {
    pub restore_image: PathBuf,
    pub configuration: MacOsVirtualMachineConfiguration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MacOsInstallationError {
    UnsupportedHost,
    InvalidPath(&'static str),
    InvalidConfiguration(&'static str),
    Native {
        domain: String,
        code: i64,
        message: String,
    },
}

impl fmt::Display for MacOsInstallationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedHost => formatter
                .write_str("this host cannot install a Virtualization.framework macOS guest"),
            Self::InvalidPath(name) => write!(formatter, "the {name} path is invalid"),
            Self::InvalidConfiguration(name) => {
                write!(formatter, "the macOS {name} configuration is invalid")
            }
            Self::Native {
                domain,
                code,
                message,
            } => write!(formatter, "{message} ({domain}, code {code})"),
        }
    }
}

impl std::error::Error for MacOsInstallationError {}

#[cfg(target_os = "macos")]
pub fn install_macos(
    request: MacOsInstallationRequest,
    progress: impl FnMut(f64) + Send + 'static,
    completion: impl FnOnce(Result<(), MacOsInstallationError>) + Send + 'static,
) -> Result<(), MacOsInstallationError> {
    use std::ffi::c_void;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::Mutex;

    type Progress = Box<dyn FnMut(f64) + Send + 'static>;
    type Completion = Box<dyn FnOnce(Result<(), MacOsInstallationError>) + Send + 'static>;

    struct InstallationContext {
        progress: Mutex<Progress>,
        completion: Option<Completion>,
    }

    unsafe extern "C" fn installation_progress(context: *mut c_void, fraction_completed: f64) {
        if context.is_null() {
            return;
        }
        // SAFETY: The bridge owns this context through completion and invokes
        // progress serially.
        let context = unsafe { &*context.cast::<InstallationContext>() };
        let Ok(mut progress) = context.progress.lock() else {
            return;
        };
        let _ = catch_unwind(AssertUnwindSafe(|| {
            progress(fraction_completed.clamp(0.0, 1.0))
        }));
    }

    unsafe extern "C" fn installation_completed(
        context: *mut c_void,
        error: *const ffi::WardVzError,
    ) {
        if context.is_null() {
            return;
        }
        // SAFETY: This pointer was produced by Box::into_raw below, and the
        // bridge invokes completion exactly once after progress ends.
        let mut context = unsafe { Box::from_raw(context.cast::<InstallationContext>()) };
        let result = if error.is_null() {
            Ok(())
        } else {
            // SAFETY: Bridge values remain valid for this callback.
            let error = unsafe { &*error };
            Err(MacOsInstallationError::Native {
                // SAFETY: Bridge strings remain valid for this callback.
                domain: unsafe { copy_bridge_string(error.domain) },
                code: error.code,
                // SAFETY: Bridge strings remain valid for this callback.
                message: unsafe { copy_bridge_string(error.message) },
            })
        };
        if let Some(completion) = context.completion.take() {
            let _ = catch_unwind(AssertUnwindSafe(|| completion(result)));
        }
    }

    let restore_image = path_to_c_string(&request.restore_image, "restore image", false)
        .map_err(MacOsInstallationError::InvalidPath)?;
    let native = NativeMacOsVirtualMachineConfiguration::new(&request.configuration)
        .map_err(MacOsInstallationError::InvalidConfiguration)?;
    if !is_supported() {
        return Err(MacOsInstallationError::UnsupportedHost);
    }
    let context = Box::new(InstallationContext {
        progress: Mutex::new(Box::new(progress)),
        completion: Some(Box::new(completion)),
    });
    let context = Box::into_raw(context).cast();

    native.with_raw(&request.configuration, |configuration| {
        // SAFETY: The bridge copies the configuration and restore path before
        // returning, then owns the callback context through completion.
        unsafe {
            ffi::ward_vz_install_macos(
                restore_image.as_ptr(),
                configuration,
                installation_progress,
                installation_completed,
                context,
            );
        }
    });
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn install_macos(
    _request: MacOsInstallationRequest,
    _progress: impl FnMut(f64) + Send + 'static,
    _completion: impl FnOnce(Result<(), MacOsInstallationError>) + Send + 'static,
) -> Result<(), MacOsInstallationError> {
    Err(MacOsInstallationError::UnsupportedHost)
}
