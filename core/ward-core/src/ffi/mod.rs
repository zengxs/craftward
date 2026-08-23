// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

mod buffer;
mod cli;
mod codex;
mod error;
mod hash;
#[cfg(feature = "app")]
mod markup;
mod realm;
mod runtime;

pub use buffer::WardBuffer;
#[cfg(feature = "app")]
pub use buffer::WardOwnedBuffer;
pub use error::{WardError, ward_core_error_destroy, ward_core_error_message};
pub use realm::*;
pub use runtime::WardRuntime;
