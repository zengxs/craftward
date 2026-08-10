// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "vz.h"

#include "errors.h"
#include "macos_bundle.h"
#include "macos_installer.h"
#include "macos_vm.h"

#include <climits>

#import <Foundation/Foundation.h>
#import <Virtualization/Virtualization.h>

namespace {

NSString* const WardVzErrorDomain = @"app.craftward.ward-realm-vz";

template<typename Callback>
void
WardVzWithBridgeError(NSError* error, Callback callback)
{
    NSString* domain = error.domain != nil ? error.domain : WardVzErrorDomain;
    NSString* message = error.localizedDescription != nil ? error.localizedDescription
                                                          : @"The native bridge failed without an error message.";
    WardVzError bridgeError = {
        .domain = domain.UTF8String,
        .code = static_cast<int64_t>(error.code),
        .message = message.UTF8String,
    };
    callback(&bridgeError);
}

void
WardVzCompleteMacOSVirtualMachineCreation(WardVzCreateMacOSVirtualMachineCompletion completion,
                                          void* context,
                                          WardVzMacOSVirtualMachineHandle* virtualMachine,
                                          const WardVzMacOSVirtualMachineStatus* status,
                                          NSError* error)
{
    if (error == nil) {
        completion(context, virtualMachine, status, nullptr);
        return;
    }

    WardVzWithBridgeError(error, [completion, context](const WardVzError* bridgeError) {
        completion(context, nullptr, nullptr, bridgeError);
    });
}

void
WardVzCompleteMacOSVirtualMachineDisplayCreation(WardVzCreateMacOSVirtualMachineDisplayCompletion completion,
                                                 void* context,
                                                 WardVzMacOSVirtualMachineDisplayHandle* display,
                                                 void* nativeView,
                                                 NSError* error)
{
    if (error == nil) {
        completion(context, display, nativeView, nullptr);
        return;
    }

    WardVzWithBridgeError(error, [completion, context](const WardVzError* bridgeError) {
        completion(context, nullptr, nullptr, bridgeError);
    });
}

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
    WardVzWithBridgeError(
      error, [completion, context](const WardVzError* bridgeError) { completion(context, nullptr, bridgeError); });
}

void
WardVzCompleteMacOSInstallation(WardVzInstallMacOSBundleCompletion completion, void* context, NSError* error)
{
    if (error == nil) {
        completion(context, nullptr);
        return;
    }

    WardVzWithBridgeError(error,
                          [completion, context](const WardVzError* bridgeError) { completion(context, bridgeError); });
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

void
ward_vz_create_macos_virtual_machine(const char* bundlePath,
                                     WardVzMacOSVirtualMachineEvent event,
                                     void* eventContext,
                                     WardVzCreateMacOSVirtualMachineCompletion completion,
                                     void* completionContext)
{
    if (completion == nullptr) {
        return;
    }

    @autoreleasepool {
#if defined(__arm64__)
        if (@available(macOS 12.0, *)) {
            if (!VZVirtualMachine.isSupported) {
                WardVzCompleteMacOSVirtualMachineCreation(
                  completion,
                  completionContext,
                  nullptr,
                  nullptr,
                  WardVzMakeError(WardVzErrorCode::UnsupportedHost,
                                  @"Virtualization.framework is unavailable on this host."));
                return;
            }
            if (bundlePath == nullptr || event == nullptr) {
                WardVzCompleteMacOSVirtualMachineCreation(
                  completion,
                  completionContext,
                  nullptr,
                  nullptr,
                  WardVzMakeError(WardVzErrorCode::InvalidArgument,
                                  @"The virtual machine creation arguments are invalid."));
                return;
            }

            NSURL* bundleURL = [NSURL fileURLWithFileSystemRepresentation:bundlePath isDirectory:YES relativeToURL:nil];
            if (bundleURL == nil) {
                WardVzCompleteMacOSVirtualMachineCreation(
                  completion,
                  completionContext,
                  nullptr,
                  nullptr,
                  WardVzMakeError(WardVzErrorCode::InvalidArgument, @"The realm bundle path is invalid."));
                return;
            }

            @try {
                NSError* error = nil;
                WardVzMacOSVirtualMachine* virtualMachine =
                  [WardVzMacOSVirtualMachine openInstalledBundleAtURL:bundleURL
                                                                event:event
                                                              context:eventContext
                                                                error:&error];
                if (virtualMachine == nil) {
                    WardVzCompleteMacOSVirtualMachineCreation(completion, completionContext, nullptr, nullptr, error);
                    return;
                }

                WardVzMacOSVirtualMachineStatus status = virtualMachine.status;
                WardVzMacOSVirtualMachineHandle* handle =
                  (__bridge_retained WardVzMacOSVirtualMachineHandle*)virtualMachine;
                WardVzCompleteMacOSVirtualMachineCreation(completion, completionContext, handle, &status, nil);
            } @catch (NSException* exception) {
                NSString* message = exception.reason != nil ? exception.reason : exception.name;
                WardVzCompleteMacOSVirtualMachineCreation(completion,
                                                          completionContext,
                                                          nullptr,
                                                          nullptr,
                                                          WardVzMakeError(WardVzErrorCode::BridgeException, message));
            }
            return;
        }
#else
        (void)bundlePath;
        (void)event;
        (void)eventContext;
#endif
        WardVzCompleteMacOSVirtualMachineCreation(
          completion,
          completionContext,
          nullptr,
          nullptr,
          WardVzMakeError(WardVzErrorCode::UnsupportedHost,
                          @"This host cannot run a Virtualization.framework macOS guest."));
    }
}

void
ward_vz_destroy_macos_virtual_machine(WardVzMacOSVirtualMachineHandle* virtualMachine)
{
#if defined(__arm64__)
    if (virtualMachine != nullptr) {
        @autoreleasepool {
            WardVzMacOSVirtualMachine* machine = (__bridge_transfer WardVzMacOSVirtualMachine*)virtualMachine;
            [machine invalidate];
        }
    }
#else
    (void)virtualMachine;
#endif
}

void
ward_vz_start_macos_virtual_machine(WardVzMacOSVirtualMachineHandle* virtualMachine)
{
#if defined(__arm64__)
    [(__bridge WardVzMacOSVirtualMachine*)virtualMachine start];
#else
    (void)virtualMachine;
#endif
}

void
ward_vz_pause_macos_virtual_machine(WardVzMacOSVirtualMachineHandle* virtualMachine)
{
#if defined(__arm64__)
    [(__bridge WardVzMacOSVirtualMachine*)virtualMachine pause];
#else
    (void)virtualMachine;
#endif
}

void
ward_vz_resume_macos_virtual_machine(WardVzMacOSVirtualMachineHandle* virtualMachine)
{
#if defined(__arm64__)
    [(__bridge WardVzMacOSVirtualMachine*)virtualMachine resume];
#else
    (void)virtualMachine;
#endif
}

void
ward_vz_request_stop_macos_virtual_machine(WardVzMacOSVirtualMachineHandle* virtualMachine)
{
#if defined(__arm64__)
    [(__bridge WardVzMacOSVirtualMachine*)virtualMachine requestStop];
#else
    (void)virtualMachine;
#endif
}

void
ward_vz_force_stop_macos_virtual_machine(WardVzMacOSVirtualMachineHandle* virtualMachine)
{
#if defined(__arm64__)
    [(__bridge WardVzMacOSVirtualMachine*)virtualMachine forceStop];
#else
    (void)virtualMachine;
#endif
}

void
ward_vz_create_macos_virtual_machine_display(WardVzMacOSVirtualMachineHandle* virtualMachine,
                                             WardVzCreateMacOSVirtualMachineDisplayCompletion completion,
                                             void* context)
{
    if (completion == nullptr) {
        return;
    }

    @autoreleasepool {
#if defined(__arm64__)
        if (@available(macOS 12.0, *)) {
            if (virtualMachine == nullptr) {
                WardVzCompleteMacOSVirtualMachineDisplayCreation(
                  completion,
                  context,
                  nullptr,
                  nullptr,
                  WardVzMakeError(WardVzErrorCode::InvalidArgument, @"The virtual machine handle is missing."));
                return;
            }

            @try {
                NSError* error = nil;
                VZVirtualMachineView* view =
                  [(__bridge WardVzMacOSVirtualMachine*)virtualMachine makeDisplayViewWithError:&error];
                if (view == nil) {
                    WardVzCompleteMacOSVirtualMachineDisplayCreation(completion, context, nullptr, nullptr, error);
                    return;
                }

                WardVzMacOSVirtualMachineDisplayHandle* handle =
                  (__bridge_retained WardVzMacOSVirtualMachineDisplayHandle*)view;
                WardVzCompleteMacOSVirtualMachineDisplayCreation(
                  completion, context, handle, (__bridge void*)view, nil);
            } @catch (NSException* exception) {
                NSString* message = exception.reason != nil ? exception.reason : exception.name;
                WardVzCompleteMacOSVirtualMachineDisplayCreation(
                  completion, context, nullptr, nullptr, WardVzMakeError(WardVzErrorCode::BridgeException, message));
            }
            return;
        }
#else
        (void)virtualMachine;
#endif
        WardVzCompleteMacOSVirtualMachineDisplayCreation(
          completion,
          context,
          nullptr,
          nullptr,
          WardVzMakeError(WardVzErrorCode::UnsupportedHost,
                          @"This host cannot display a Virtualization.framework macOS guest."));
    }
}

void
ward_vz_destroy_macos_virtual_machine_display(WardVzMacOSVirtualMachineDisplayHandle* display)
{
#if defined(__arm64__)
    if (display != nullptr) {
        @autoreleasepool {
            VZVirtualMachineView* view = (__bridge_transfer VZVirtualMachineView*)display;
            view.virtualMachine = nil;
        }
    }
#else
    (void)display;
#endif
}
