// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "vz.h"

#if defined(__arm64__)

#import <Foundation/Foundation.h>

@class VZVirtualMachineConfiguration;

NS_ASSUME_NONNULL_BEGIN

@interface WardVzMacOSBundle : NSObject

+ (instancetype)new NS_UNAVAILABLE;
- (instancetype)init NS_UNAVAILABLE;

+ (nullable instancetype)openPreparedBundleAtURL:(NSURL*)URL error:(NSError**)error;

+ (nullable instancetype)openInstalledBundleAtURL:(NSURL*)URL error:(NSError**)error;

- (nullable VZVirtualMachineConfiguration*)createVirtualMachineConfigurationWithError:(NSError**)error;

- (BOOL)hasSavedMachineState;

- (nullable NSURL*)beginSavingMachineStateWithError:(NSError**)error;

- (BOOL)finishSavingMachineStateAtURL:(NSURL*)URL error:(NSError**)error;

- (void)cancelSavingMachineStateAtURL:(NSURL*)URL;

- (nullable NSURL*)consumeSavedMachineStateWithError:(NSError**)error;

- (BOOL)finishConsumingMachineStateAtURL:(NSURL*)URL error:(NSError**)error;

- (BOOL)discardSavedMachineStateWithError:(NSError**)error;

- (BOOL)transitionToInstallingWithError:(NSError**)error;

- (BOOL)transitionToInstalledWithError:(NSError**)error;

- (void)transitionToInstallationFailed;

@end

void
WardVzStartPreparingMacOSBundle(NSURL* restoreImageURL,
                                NSURL* destinationURL,
                                uint64_t diskSize,
                                WardVzPrepareMacOSBundleCompletion completion,
                                void* context);

NS_ASSUME_NONNULL_END

#endif
