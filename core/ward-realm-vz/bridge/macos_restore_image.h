// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "vz.h"

#if defined(__arm64__)

#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

void
WardVzStartPreparingMacOS(NSURL* restoreImageURL,
                          NSURL* diskURL,
                          NSURL* auxiliaryStorageURL,
                          uint64_t diskSize,
                          WardVzPrepareMacOSCompletion completion,
                          void* context);

NS_ASSUME_NONNULL_END

#endif
