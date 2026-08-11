// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "macos_machine_state.h"

#include "errors.h"

#if defined(__arm64__)

@interface WardVzMacOSMachineState ()

@property (nonatomic, copy) NSURL* machineStateURL;
@property (nonatomic, copy) NSURL* savingStateURL;
@property (nonatomic, copy) NSURL* restoringStateURL;

@end

@implementation WardVzMacOSMachineState

- (instancetype)initWithMachineStateURL:(NSURL*)machineStateURL
                       savingStateAtURL:(NSURL*)savingStateURL
                    restoringStateAtURL:(NSURL*)restoringStateURL
{
    self = [super init];
    if (self != nil) {
        _machineStateURL = [machineStateURL copy];
        _savingStateURL = [savingStateURL copy];
        _restoringStateURL = [restoringStateURL copy];

        // Interrupted saves are incomplete, while interrupted restores have
        // consumed state that may no longer match the guest disk.
        NSFileManager* fileManager = [NSFileManager defaultManager];
        [fileManager removeItemAtURL:_savingStateURL error:nil];
        [fileManager removeItemAtURL:_restoringStateURL error:nil];
    }
    return self;
}

- (BOOL)hasSavedMachineState
{
    return [[NSFileManager defaultManager] fileExistsAtPath:self.machineStateURL.path];
}

- (NSURL*)beginSavingWithError:(NSError**)error
{
    NSFileManager* fileManager = [NSFileManager defaultManager];
    if (self.hasSavedMachineState) {
        if (error != nullptr) {
            *error =
              WardVzMakeError(WardVzErrorCode::InvalidState, @"The virtual machine already has a suspended state.");
        }
        return nil;
    }

    NSURL* parentURL = self.savingStateURL.URLByDeletingLastPathComponent;
    if (![fileManager createDirectoryAtURL:parentURL
               withIntermediateDirectories:YES
                                attributes:@{ NSFilePosixPermissions : @0700 }
                                     error:error]) {
        return nil;
    }
    if ([fileManager fileExistsAtPath:self.savingStateURL.path] && ![fileManager removeItemAtURL:self.savingStateURL
                                                                                           error:error]) {
        return nil;
    }
    return self.savingStateURL;
}

- (BOOL)finishSavingAtURL:(NSURL*)URL error:(NSError**)error
{
    if (![URL isEqual:self.savingStateURL]) {
        if (error != nullptr) {
            *error = WardVzMakeError(WardVzErrorCode::InvalidArgument,
                                     @"The temporary machine state does not belong to this virtual machine.");
        }
        return NO;
    }
    return [[NSFileManager defaultManager] moveItemAtURL:URL toURL:self.machineStateURL error:error];
}

- (void)cancelSavingAtURL:(NSURL*)URL
{
    if ([URL isEqual:self.savingStateURL]) {
        [[NSFileManager defaultManager] removeItemAtURL:URL error:nil];
    }
}

- (NSURL*)consumeWithError:(NSError**)error
{
    NSFileManager* fileManager = [NSFileManager defaultManager];
    if (![fileManager fileExistsAtPath:self.machineStateURL.path]) {
        if (error != nullptr) {
            *error =
              WardVzMakeError(WardVzErrorCode::InvalidState, @"The virtual machine has no suspended state to restore.");
        }
        return nil;
    }

    if ([fileManager fileExistsAtPath:self.restoringStateURL.path] &&
        ![fileManager removeItemAtURL:self.restoringStateURL error:error]) {
        return nil;
    }
    // A restore attempt makes the state unsafe to reuse even if VZ reports an
    // error, so remove it from the set of resumable states first.
    if (![fileManager moveItemAtURL:self.machineStateURL toURL:self.restoringStateURL error:error]) {
        return nil;
    }
    return self.restoringStateURL;
}

- (BOOL)finishConsumingAtURL:(NSURL*)URL error:(NSError**)error
{
    if (![URL isEqual:self.restoringStateURL]) {
        if (error != nullptr) {
            *error = WardVzMakeError(WardVzErrorCode::InvalidArgument,
                                     @"The restoring machine state does not belong to this virtual machine.");
        }
        return NO;
    }
    return [[NSFileManager defaultManager] removeItemAtURL:URL error:error];
}

- (BOOL)discardWithError:(NSError**)error
{
    NSFileManager* fileManager = [NSFileManager defaultManager];
    if (![fileManager fileExistsAtPath:self.machineStateURL.path]) {
        return YES;
    }
    return [fileManager removeItemAtURL:self.machineStateURL error:error];
}

@end

#endif
