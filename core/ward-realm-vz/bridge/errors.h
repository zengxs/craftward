// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "vz.h"

#import <Foundation/Foundation.h>

enum class WardVzErrorCode : NSInteger
{
    InvalidArgument = 1,
    UnsupportedHost = 2,
    UnsupportedRestoreImage = 3,
    DestinationExists = 4,
    BridgeException = 5,
    InvalidBundle = 6,
    InvalidBundleState = 7,
};

NSError*
WardVzMakeError(WardVzErrorCode code, NSString* message);

void
WardVzCompleteBundlePreparationWithError(WardVzPrepareMacOSBundleCompletion completion, void* context, NSError* error);

void
WardVzCompleteMacOSInstallation(WardVzInstallMacOSBundleCompletion completion, void* context, NSError* error);
