// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "vz.h"

#if defined(__arm64__)

#import <Foundation/Foundation.h>

@class VZVirtualMachineConfiguration;

NS_ASSUME_NONNULL_BEGIN

void
WardVzStartInstallingMacOS(NSURL* restoreImageURL,
                           VZVirtualMachineConfiguration* configuration,
                           WardVzMacOSInstallationProgress progress,
                           WardVzInstallMacOSCompletion completion,
                           void* context);

NS_ASSUME_NONNULL_END

#endif
