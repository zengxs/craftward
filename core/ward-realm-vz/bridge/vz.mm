// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "vz.h"

#include "errors.h"
#include "macos_configuration.h"
#include "macos_installer.h"
#include "macos_machine_state.h"
#include "macos_restore_image.h"
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
WardVzCompleteMacOSPreparationWithError(WardVzPrepareMacOSCompletion completion, void* context, NSError* error)
{
    WardVzWithBridgeError(
      error, [completion, context](const WardVzError* bridgeError) { completion(context, nullptr, bridgeError); });
}

void
WardVzCompleteMacOSInstallation(WardVzInstallMacOSCompletion completion, void* context, NSError* error)
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
ward_vz_prepare_macos(const char* restoreImagePath,
                      const char* diskPath,
                      const char* auxiliaryStoragePath,
                      uint64_t diskSize,
                      WardVzPrepareMacOSCompletion completion,
                      void* context)
{
    if (completion == nullptr) {
        return;
    }

    @autoreleasepool {
#if defined(__arm64__)
        if (@available(macOS 12.0, *)) {
            if (!VZVirtualMachine.isSupported) {
                WardVzCompleteMacOSPreparationWithError(
                  completion,
                  context,
                  WardVzMakeError(WardVzErrorCode::UnsupportedHost,
                                  @"Virtualization.framework is unavailable on this host."));
                return;
            }
            if (restoreImagePath == nullptr || diskPath == nullptr || auxiliaryStoragePath == nullptr ||
                diskSize == 0 || diskSize % 512 != 0 || diskSize > static_cast<uint64_t>(LLONG_MAX)) {
                WardVzCompleteMacOSPreparationWithError(
                  completion,
                  context,
                  WardVzMakeError(WardVzErrorCode::InvalidArgument, @"The macOS preparation arguments are invalid."));
                return;
            }

            NSURL* restoreImageURL = [NSURL fileURLWithFileSystemRepresentation:restoreImagePath
                                                                    isDirectory:NO
                                                                  relativeToURL:nil];
            NSURL* diskURL = [NSURL fileURLWithFileSystemRepresentation:diskPath isDirectory:NO relativeToURL:nil];
            NSURL* auxiliaryStorageURL = [NSURL fileURLWithFileSystemRepresentation:auxiliaryStoragePath
                                                                        isDirectory:NO
                                                                      relativeToURL:nil];
            if (restoreImageURL == nil || diskURL == nil || auxiliaryStorageURL == nil) {
                WardVzCompleteMacOSPreparationWithError(
                  completion,
                  context,
                  WardVzMakeError(WardVzErrorCode::InvalidArgument, @"A macOS preparation path is invalid."));
                return;
            }

            WardVzStartPreparingMacOS(restoreImageURL, diskURL, auxiliaryStorageURL, diskSize, completion, context);
            return;
        }
#else
        (void)restoreImagePath;
        (void)diskPath;
        (void)auxiliaryStoragePath;
        (void)diskSize;
#endif
        WardVzCompleteMacOSPreparationWithError(
          completion,
          context,
          WardVzMakeError(WardVzErrorCode::UnsupportedHost,
                          @"This host cannot create a Virtualization.framework macOS guest."));
    }
}

void
ward_vz_install_macos(const char* restoreImagePath,
                      const WardVzMacOSVirtualMachineConfiguration* configuration,
                      WardVzMacOSInstallationProgress progress,
                      WardVzInstallMacOSCompletion completion,
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
            if (restoreImagePath == nullptr || configuration == nullptr) {
                WardVzCompleteMacOSInstallation(
                  completion,
                  context,
                  WardVzMakeError(WardVzErrorCode::InvalidArgument, @"The macOS installation arguments are invalid."));
                return;
            }

            NSURL* restoreImageURL = [NSURL fileURLWithFileSystemRepresentation:restoreImagePath
                                                                    isDirectory:NO
                                                                  relativeToURL:nil];
            if (restoreImageURL == nil) {
                WardVzCompleteMacOSInstallation(
                  completion,
                  context,
                  WardVzMakeError(WardVzErrorCode::InvalidArgument, @"A macOS installation path is invalid."));
                return;
            }

            @try {
                NSError* error = nil;
                WardVzMacOSConfiguration* configurationSource =
                  [[WardVzMacOSConfiguration alloc] initWithConfiguration:configuration error:&error];
                VZVirtualMachineConfiguration* virtualMachineConfiguration =
                  [configurationSource makeVirtualMachineConfigurationWithError:&error];
                if (virtualMachineConfiguration == nil) {
                    WardVzCompleteMacOSInstallation(completion, context, error);
                    return;
                }

                WardVzStartInstallingMacOS(restoreImageURL, virtualMachineConfiguration, progress, completion, context);
            } @catch (NSException* exception) {
                NSString* message = exception.reason != nil ? exception.reason : exception.name;
                WardVzCompleteMacOSInstallation(
                  completion, context, WardVzMakeError(WardVzErrorCode::BridgeException, message));
            }
            return;
        }
#else
        (void)restoreImagePath;
        (void)configuration;
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
ward_vz_create_macos_virtual_machine(const WardVzMacOSVirtualMachineConfiguration* configuration,
                                     const char* machineStatePath,
                                     const char* savingMachineStatePath,
                                     const char* restoringMachineStatePath,
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
            if (configuration == nullptr || machineStatePath == nullptr || savingMachineStatePath == nullptr ||
                restoringMachineStatePath == nullptr || event == nullptr) {
                WardVzCompleteMacOSVirtualMachineCreation(
                  completion,
                  completionContext,
                  nullptr,
                  nullptr,
                  WardVzMakeError(WardVzErrorCode::InvalidArgument,
                                  @"The virtual machine creation arguments are invalid."));
                return;
            }

            NSURL* machineStateURL = [NSURL fileURLWithFileSystemRepresentation:machineStatePath
                                                                    isDirectory:NO
                                                                  relativeToURL:nil];
            NSURL* savingMachineStateURL = [NSURL fileURLWithFileSystemRepresentation:savingMachineStatePath
                                                                          isDirectory:NO
                                                                        relativeToURL:nil];
            NSURL* restoringMachineStateURL = [NSURL fileURLWithFileSystemRepresentation:restoringMachineStatePath
                                                                             isDirectory:NO
                                                                           relativeToURL:nil];
            if (machineStateURL == nil || savingMachineStateURL == nil || restoringMachineStateURL == nil) {
                WardVzCompleteMacOSVirtualMachineCreation(
                  completion,
                  completionContext,
                  nullptr,
                  nullptr,
                  WardVzMakeError(WardVzErrorCode::InvalidArgument, @"A machine state path is invalid."));
                return;
            }

            @try {
                NSError* error = nil;
                WardVzMacOSConfiguration* configurationSource =
                  [[WardVzMacOSConfiguration alloc] initWithConfiguration:configuration error:&error];
                if (configurationSource == nil) {
                    WardVzCompleteMacOSVirtualMachineCreation(completion, completionContext, nullptr, nullptr, error);
                    return;
                }
                WardVzMacOSMachineState* machineState =
                  [[WardVzMacOSMachineState alloc] initWithMachineStateURL:machineStateURL
                                                          savingStateAtURL:savingMachineStateURL
                                                       restoringStateAtURL:restoringMachineStateURL];
                WardVzMacOSVirtualMachine* virtualMachine =
                  [WardVzMacOSVirtualMachine createWithConfiguration:configurationSource
                                                        machineState:machineState
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
        (void)configuration;
        (void)machineStatePath;
        (void)savingMachineStatePath;
        (void)restoringMachineStatePath;
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
ward_vz_suspend_macos_virtual_machine(WardVzMacOSVirtualMachineHandle* virtualMachine)
{
#if defined(__arm64__)
    [(__bridge WardVzMacOSVirtualMachine*)virtualMachine suspend];
#else
    (void)virtualMachine;
#endif
}

void
ward_vz_restore_macos_virtual_machine(WardVzMacOSVirtualMachineHandle* virtualMachine)
{
#if defined(__arm64__)
    [(__bridge WardVzMacOSVirtualMachine*)virtualMachine restore];
#else
    (void)virtualMachine;
#endif
}

void
ward_vz_discard_macos_virtual_machine_saved_state(WardVzMacOSVirtualMachineHandle* virtualMachine)
{
#if defined(__arm64__)
    [(__bridge WardVzMacOSVirtualMachine*)virtualMachine discardSavedState];
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
