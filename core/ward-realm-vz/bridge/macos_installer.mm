// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "macos_installer.h"

#include "errors.h"
#include "macos_bundle.h"

#import <Virtualization/Virtualization.h>

#if defined(__arm64__)

void
WardVzStartInstallingMacOSBundle(NSURL* restoreImageURL,
                                 NSURL* bundleURL,
                                 WardVzMacOSInstallationProgress progress,
                                 WardVzInstallMacOSBundleCompletion completion,
                                 void* context)
{
    dispatch_queue_t queue = dispatch_queue_create("app.craftward.ward-realm-vz.macos", DISPATCH_QUEUE_SERIAL);
    dispatch_async(queue, ^{
      @autoreleasepool {
          WardVzMacOSBundle* bundle = nil;
          BOOL installationStarted = NO;
          @try {
              NSError* error = nil;
              bundle = [WardVzMacOSBundle openPreparedBundleAtURL:bundleURL error:&error];
              if (bundle == nil) {
                  WardVzCompleteMacOSInstallation(completion, context, error);
                  return;
              }

              VZVirtualMachineConfiguration* configuration = [bundle createVirtualMachineConfigurationWithError:&error];
              if (configuration == nil) {
                  WardVzCompleteMacOSInstallation(completion, context, error);
                  return;
              }

              VZVirtualMachine* virtualMachine = [[VZVirtualMachine alloc] initWithConfiguration:configuration
                                                                                           queue:queue];
              VZMacOSInstaller* installer = [[VZMacOSInstaller alloc] initWithVirtualMachine:virtualMachine
                                                                             restoreImageURL:restoreImageURL];

              if (![bundle transitionToInstallingWithError:&error]) {
                  WardVzCompleteMacOSInstallation(completion, context, error);
                  return;
              }
              installationStarted = YES;

              __block BOOL completed = NO;
              dispatch_source_t progressTimer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, queue);
              dispatch_source_set_timer(
                progressTimer, dispatch_time(DISPATCH_TIME_NOW, 0), 250 * NSEC_PER_MSEC, 25 * NSEC_PER_MSEC);
              dispatch_source_set_event_handler(progressTimer, ^{
                if (!completed && progress != nullptr) {
                    progress(context, installer.progress.fractionCompleted);
                }
              });
              dispatch_resume(progressTimer);

              @try {
                  [installer installWithCompletionHandler:^(NSError* installationError) {
                    @autoreleasepool {
                        completed = YES;
                        dispatch_source_cancel(progressTimer);

                        @try {
                            NSError* resultError = installationError;
                            if (installationError == nil) {
                                if (![bundle transitionToInstalledWithError:&resultError]) {
                                    // The guest is installed, but callers must not use
                                    // a bundle whose durable state was not recorded.
                                } else if (progress != nullptr) {
                                    progress(context, 1.0);
                                }
                            } else {
                                [bundle transitionToInstallationFailed];
                            }
                            WardVzCompleteMacOSInstallation(completion, context, resultError);
                        } @catch (NSException* exception) {
                            [bundle transitionToInstallationFailed];
                            NSString* message = exception.reason != nil ? exception.reason : exception.name;
                            WardVzCompleteMacOSInstallation(
                              completion, context, WardVzMakeError(WardVzErrorCode::BridgeException, message));
                        }
                    }
                  }];
              } @catch (NSException* exception) {
                  completed = YES;
                  dispatch_source_cancel(progressTimer);
                  @throw;
              }
          } @catch (NSException* exception) {
              if (installationStarted) {
                  [bundle transitionToInstallationFailed];
              }
              NSString* message = exception.reason != nil ? exception.reason : exception.name;
              WardVzCompleteMacOSInstallation(
                completion, context, WardVzMakeError(WardVzErrorCode::BridgeException, message));
          }
      }
    });
}

#endif
