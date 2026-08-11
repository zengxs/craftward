// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "macos_installer.h"

#include "errors.h"

#import <Virtualization/Virtualization.h>

#if defined(__arm64__)

void
WardVzStartInstallingMacOS(NSURL* restoreImageURL,
                           VZVirtualMachineConfiguration* configuration,
                           WardVzMacOSInstallationProgress progress,
                           WardVzInstallMacOSCompletion completion,
                           void* context)
{
    dispatch_queue_t queue = dispatch_queue_create("app.craftward.ward-realm-vz.macos", DISPATCH_QUEUE_SERIAL);
    dispatch_async(queue, ^{
      @autoreleasepool {
          @try {
              VZVirtualMachine* virtualMachine = [[VZVirtualMachine alloc] initWithConfiguration:configuration
                                                                                           queue:queue];
              VZMacOSInstaller* installer = [[VZMacOSInstaller alloc] initWithVirtualMachine:virtualMachine
                                                                             restoreImageURL:restoreImageURL];

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
                        if (installationError == nil && progress != nullptr) {
                            progress(context, 1.0);
                        }
                        WardVzCompleteMacOSInstallation(completion, context, installationError);
                    }
                  }];
              } @catch (NSException* exception) {
                  completed = YES;
                  dispatch_source_cancel(progressTimer);
                  @throw;
              }
          } @catch (NSException* exception) {
              NSString* message = exception.reason != nil ? exception.reason : exception.name;
              WardVzCompleteMacOSInstallation(
                completion, context, WardVzMakeError(WardVzErrorCode::BridgeException, message));
          }
      }
    });
}

#endif
