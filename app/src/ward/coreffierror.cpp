// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/coreffierror.h"

#include "ward/coreffi.h"

#include <memory>

namespace {
struct WardErrorDeleter
{
    void operator()(WardError* error) const { ward_core_error_destroy(error); }
};
}

QString
ward::coreffi::takeErrorMessage(WardError* error)
{
    const std::unique_ptr<WardError, WardErrorDeleter> ownedError(error);
    if (!ownedError)
        return {};
    const char* message = ward_core_error_message(ownedError.get());
    return message == nullptr ? QString() : QString::fromUtf8(message);
}
