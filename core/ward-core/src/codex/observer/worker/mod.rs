// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

mod actor;
mod operation;
mod polling;
mod state;
mod writer;

#[cfg(test)]
mod tests;

pub(super) use actor::run_observer;
