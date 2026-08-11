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
    BridgeException = 5,
    InvalidConfiguration = 6,
    InvalidState = 7,
};

NSError*
WardVzMakeError(WardVzErrorCode code, NSString* message);

void
WardVzCompleteMacOSPreparationWithError(WardVzPrepareMacOSCompletion completion, void* context, NSError* error);

void
WardVzCompleteMacOSInstallation(WardVzInstallMacOSCompletion completion, void* context, NSError* error);
