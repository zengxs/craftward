// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#if defined(__arm64__)

#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

@interface WardVzMacOSMachineState : NSObject

+ (instancetype)new NS_UNAVAILABLE;
- (instancetype)init NS_UNAVAILABLE;

- (instancetype)initWithMachineStateURL:(NSURL*)machineStateURL
                       savingStateAtURL:(NSURL*)savingStateURL
                    restoringStateAtURL:(NSURL*)restoringStateURL NS_DESIGNATED_INITIALIZER;

@property (nonatomic, readonly) BOOL hasSavedMachineState;

- (nullable NSURL*)beginSavingWithError:(NSError**)error;
- (BOOL)finishSavingAtURL:(NSURL*)URL error:(NSError**)error;
- (void)cancelSavingAtURL:(NSURL*)URL;
- (nullable NSURL*)consumeWithError:(NSError**)error;
- (BOOL)finishConsumingAtURL:(NSURL*)URL error:(NSError**)error;
- (BOOL)discardWithError:(NSError**)error;

@end

NS_ASSUME_NONNULL_END

#endif
