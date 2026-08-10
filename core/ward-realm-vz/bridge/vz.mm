// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "vz.h"

#include "errors.h"
#include "macos_bundle.h"
#include "macos_installer.h"

#include <climits>

#import <Foundation/Foundation.h>
#import <Virtualization/Virtualization.h>

namespace {

NSString* const WardVzErrorDomain = @"app.craftward.ward-realm-vz";

} // namespace

NSError*
WardVzMakeError(WardVzErrorCode code, NSString* message)
{
    return [NSError errorWithDomain:WardVzErrorDomain
                               code:static_cast<NSInteger>(code)
                           userInfo:@{ NSLocalizedDescriptionKey : message }];
}

void
WardVzCompleteBundlePreparationWithError(WardVzPrepareMacOSBundleCompletion completion, void* context, NSError* error)
{
    NSString* domain = error.domain != nil ? error.domain : WardVzErrorDomain;
    NSString* message = error.localizedDescription != nil ? error.localizedDescription
                                                          : @"The native bridge failed without an error message.";
    WardVzError bridgeError = {
        .domain = domain.UTF8String,
        .code = static_cast<int64_t>(error.code),
        .message = message.UTF8String,
    };
    completion(context, nullptr, &bridgeError);
}

void
WardVzCompleteMacOSInstallation(WardVzInstallMacOSBundleCompletion completion, void* context, NSError* error)
{
    if (error == nil) {
        completion(context, nullptr);
        return;
    }

    NSString* domain = error.domain != nil ? error.domain : WardVzErrorDomain;
    NSString* message = error.localizedDescription != nil ? error.localizedDescription
                                                          : @"The native bridge failed without an error message.";
    WardVzError bridgeError = {
        .domain = domain.UTF8String,
        .code = static_cast<int64_t>(error.code),
        .message = message.UTF8String,
    };
    completion(context, &bridgeError);
}

bool
ward_vz_is_supported(void)
{
    @autoreleasepool {
        if (@available(macOS 11.0, *)) {
            return VZVirtualMachine.isSupported;
        }
        return false;
    }
}

void
ward_vz_prepare_macos_bundle(const char* restoreImagePath,
                             const char* destinationPath,
                             uint64_t diskSize,
                             WardVzPrepareMacOSBundleCompletion completion,
                             void* context)
{
    if (completion == nullptr) {
        return;
    }

    @autoreleasepool {
#if defined(__arm64__)
        if (@available(macOS 12.0, *)) {
            if (!VZVirtualMachine.isSupported) {
                WardVzCompleteBundlePreparationWithError(
                  completion,
                  context,
                  WardVzMakeError(WardVzErrorCode::UnsupportedHost,
                                  @"Virtualization.framework is unavailable on this host."));
                return;
            }
            if (restoreImagePath == nullptr || destinationPath == nullptr || diskSize == 0 || diskSize % 512 != 0 ||
                diskSize > static_cast<uint64_t>(LLONG_MAX)) {
                WardVzCompleteBundlePreparationWithError(
                  completion,
                  context,
                  WardVzMakeError(WardVzErrorCode::InvalidArgument, @"The bundle preparation arguments are invalid."));
                return;
            }

            NSURL* restoreImageURL = [NSURL fileURLWithFileSystemRepresentation:restoreImagePath
                                                                    isDirectory:NO
                                                                  relativeToURL:nil];
            NSURL* destinationURL = [NSURL fileURLWithFileSystemRepresentation:destinationPath
                                                                   isDirectory:YES
                                                                 relativeToURL:nil];
            if (restoreImageURL == nil || destinationURL == nil) {
                WardVzCompleteBundlePreparationWithError(
                  completion,
                  context,
                  WardVzMakeError(WardVzErrorCode::InvalidArgument, @"A bundle preparation path is invalid."));
                return;
            }

            WardVzStartPreparingMacOSBundle(restoreImageURL, destinationURL, diskSize, completion, context);
            return;
        }
#else
        (void)restoreImagePath;
        (void)destinationPath;
        (void)diskSize;
#endif
        WardVzCompleteBundlePreparationWithError(
          completion,
          context,
          WardVzMakeError(WardVzErrorCode::UnsupportedHost,
                          @"This host cannot create a Virtualization.framework macOS guest."));
    }
}

void
ward_vz_install_macos_bundle(const char* restoreImagePath,
                             const char* bundlePath,
                             WardVzMacOSInstallationProgress progress,
                             WardVzInstallMacOSBundleCompletion completion,
                             void* context)
{
    if (completion == nullptr) {
        return;
    }

    @autoreleasepool {
#if defined(__arm64__)
        if (@available(macOS 12.0, *)) {
            if (!VZVirtualMachine.isSupported) {
                WardVzCompleteMacOSInstallation(
                  completion,
                  context,
                  WardVzMakeError(WardVzErrorCode::UnsupportedHost,
                                  @"Virtualization.framework is unavailable on this host."));
                return;
            }
            if (restoreImagePath == nullptr || bundlePath == nullptr) {
                WardVzCompleteMacOSInstallation(
                  completion,
                  context,
                  WardVzMakeError(WardVzErrorCode::InvalidArgument, @"The macOS installation arguments are invalid."));
                return;
            }

            NSURL* restoreImageURL = [NSURL fileURLWithFileSystemRepresentation:restoreImagePath
                                                                    isDirectory:NO
                                                                  relativeToURL:nil];
            NSURL* bundleURL = [NSURL fileURLWithFileSystemRepresentation:bundlePath isDirectory:YES relativeToURL:nil];
            if (restoreImageURL == nil || bundleURL == nil) {
                WardVzCompleteMacOSInstallation(
                  completion,
                  context,
                  WardVzMakeError(WardVzErrorCode::InvalidArgument, @"A macOS installation path is invalid."));
                return;
            }

            WardVzStartInstallingMacOSBundle(restoreImageURL, bundleURL, progress, completion, context);
            return;
        }
#else
        (void)restoreImagePath;
        (void)bundlePath;
        (void)progress;
#endif
        WardVzCompleteMacOSInstallation(
          completion,
          context,
          WardVzMakeError(WardVzErrorCode::UnsupportedHost,
                          @"This host cannot install a Virtualization.framework macOS guest."));
    }
}
