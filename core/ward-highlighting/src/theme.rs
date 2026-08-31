// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

/// An application-maintained syntax-highlighting theme.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    pub(crate) const ALL: [Self; 2] = [Self::Light, Self::Dark];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Light => "One Light",
            Self::Dark => "One Dark",
        }
    }
}
