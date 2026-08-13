// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QQuickImageProvider>

#include <memory>

[[nodiscard]] std::unique_ptr<QQuickImageProvider>
createApplicationIconProvider();
