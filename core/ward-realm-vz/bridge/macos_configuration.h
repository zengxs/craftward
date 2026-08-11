// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "vz.h"

#if defined(__arm64__)

#import <Foundation/Foundation.h>

@class VZVirtualMachineConfiguration;

NS_ASSUME_NONNULL_BEGIN

@interface WardVzMacOSConfiguration : NSObject

+ (instancetype)new NS_UNAVAILABLE;
- (instancetype)init NS_UNAVAILABLE;

- (nullable instancetype)initWithConfiguration:(const WardVzMacOSVirtualMachineConfiguration*)configuration
                                         error:(NSError**)error NS_DESIGNATED_INITIALIZER;

- (nullable VZVirtualMachineConfiguration*)makeVirtualMachineConfigurationWithError:(NSError**)error;

@end

NS_ASSUME_NONNULL_END

#endif
