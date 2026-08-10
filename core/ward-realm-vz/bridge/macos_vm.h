// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "vz.h"

#if defined(__arm64__)

#import <Foundation/Foundation.h>
#import <Virtualization/Virtualization.h>

NS_ASSUME_NONNULL_BEGIN

@interface WardVzMacOSVirtualMachine : NSObject<VZVirtualMachineDelegate>

+ (instancetype)new NS_UNAVAILABLE;
- (instancetype)init NS_UNAVAILABLE;

+ (nullable instancetype)openInstalledBundleAtURL:(NSURL*)bundleURL
                                            event:(WardVzMacOSVirtualMachineEvent)event
                                          context:(void*)context
                                            error:(NSError**)error;

@property (nonatomic, readonly) WardVzMacOSVirtualMachineStatus status;

- (void)start;
- (void)pause;
- (void)resume;
- (void)requestStop;
- (void)forceStop;
- (nullable VZVirtualMachineView*)makeDisplayViewWithError:(NSError**)error;
- (void)invalidate;

@end

NS_ASSUME_NONNULL_END

#endif
