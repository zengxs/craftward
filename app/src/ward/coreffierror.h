// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QString>

struct WardError;

namespace ward::coreffi {
/// Takes ownership of `error` and returns its UTF-8 message, or an empty string
/// when the error or its message is null.
[[nodiscard]] QString
takeErrorMessage(WardError* error);
}
