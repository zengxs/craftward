// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::{CStr, c_char};
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::{WardError, write_error};

/// A BLAKE3-256 digest passed through Ward Core's private C interface.
#[repr(C)]
pub struct WardBlake3Digest {
    bytes: [u8; blake3::OUT_LEN],
}

fn hash_reader(mut reader: impl Read) -> io::Result<[u8; blake3::OUT_LEN]> {
    let mut hasher = blake3::Hasher::new();
    io::copy(&mut reader, &mut hasher)?;
    Ok(*hasher.finalize().as_bytes())
}

fn hash_file(path: &Path) -> io::Result<[u8; blake3::OUT_LEN]> {
    hash_reader(File::open(path)?)
}

/// Computes the BLAKE3-256 digest of a file on the calling thread.
///
/// # Safety
///
/// `path` must point to a NUL-terminated UTF-8 path. `output_digest` must be
/// writable. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_blake3_hash_file(
    path: *const c_char,
    output_digest: *mut WardBlake3Digest,
    output_error: *mut *mut WardError,
) -> bool {
    if !output_error.is_null() {
        // SAFETY: The C caller supplied a writable error output pointer.
        unsafe { *output_error = std::ptr::null_mut() };
    }
    let Some(output_digest) = (unsafe { output_digest.as_mut() }) else {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the BLAKE3 digest output is missing") };
        return false;
    };
    if path.is_null() {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the file path to hash is missing") };
        return false;
    }

    // SAFETY: The private C interface requires a NUL-terminated UTF-8 path.
    let path = unsafe { CStr::from_ptr(path) };
    let path = PathBuf::from(path.to_string_lossy().into_owned());
    match hash_file(&path) {
        Ok(bytes) => {
            output_digest.bytes = bytes;
            true
        }
        Err(error) => {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe {
                write_error(
                    output_error,
                    format!("failed to hash the file with BLAKE3: {error}"),
                )
            };
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::io::Cursor;
    use std::path::PathBuf;

    use super::{WardBlake3Digest, hash_file, hash_reader, ward_core_blake3_hash_file};

    #[test]
    fn hashes_known_bytes() {
        let digest = hash_reader(Cursor::new(b"abc")).expect("the in-memory bytes should hash");

        assert_eq!(
            hex(&digest),
            "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
        );
    }

    #[test]
    fn hashes_a_file_through_the_private_c_interface() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let encoded_path = CString::new(path.to_string_lossy().as_bytes())
            .expect("the manifest path should not contain NUL bytes");
        let mut digest = WardBlake3Digest { bytes: [0; 32] };
        let mut error = std::ptr::null_mut();

        // SAFETY: The path, digest, and error output satisfy the private C interface.
        let succeeded = unsafe {
            ward_core_blake3_hash_file(encoded_path.as_ptr(), &raw mut digest, &raw mut error)
        };

        assert!(succeeded);
        assert!(error.is_null());
        assert_eq!(
            digest.bytes,
            hash_file(&path).expect("the manifest should hash")
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
