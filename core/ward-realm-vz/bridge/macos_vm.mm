// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "macos_vm.h"

#include "errors.h"
#include "macos_configuration.h"
#include "macos_machine_state.h"

#if defined(__arm64__)

namespace {

const void* WardVzMacOSVirtualMachineQueueKey = &WardVzMacOSVirtualMachineQueueKey;

WardVzMacOSVirtualMachineState
WardVzMapVirtualMachineState(VZVirtualMachineState state)
{
    switch (state) {
        case VZVirtualMachineStateStopped:
            return WardVzMacOSVirtualMachineStateStopped;
        case VZVirtualMachineStateRunning:
            return WardVzMacOSVirtualMachineStateRunning;
        case VZVirtualMachineStatePaused:
            return WardVzMacOSVirtualMachineStatePaused;
        case VZVirtualMachineStateError:
            return WardVzMacOSVirtualMachineStateError;
        case VZVirtualMachineStateStarting:
            return WardVzMacOSVirtualMachineStateStarting;
        case VZVirtualMachineStatePausing:
            return WardVzMacOSVirtualMachineStatePausing;
        case VZVirtualMachineStateResuming:
            return WardVzMacOSVirtualMachineStateResuming;
        case VZVirtualMachineStateStopping:
            return WardVzMacOSVirtualMachineStateStopping;
        case VZVirtualMachineStateSaving:
            return WardVzMacOSVirtualMachineStateSaving;
        case VZVirtualMachineStateRestoring:
            return WardVzMacOSVirtualMachineStateRestoring;
    }
    return WardVzMacOSVirtualMachineStateError;
}

NSError*
WardVzMakeInvalidStateError(NSString* operation)
{
    return WardVzMakeError(
      WardVzErrorCode::InvalidState,
      [NSString stringWithFormat:@"The virtual machine cannot %@ from its current state.", operation]);
}

NSError*
WardVzMakeBridgeExceptionError(NSException* exception)
{
    NSString* message = exception.reason != nil ? exception.reason : exception.name;
    return WardVzMakeError(WardVzErrorCode::BridgeException, message);
}

NSError*
WardVzValidateSaveRestoreSupport(VZVirtualMachineConfiguration* configuration)
{
    if (@available(macOS 14.0, *)) {
        NSError* error = nil;
        if (![configuration validateSaveRestoreSupportWithError:&error] && error == nil) {
            return WardVzMakeError(WardVzErrorCode::InvalidConfiguration,
                                   @"The virtual machine configuration does not support saving and restoring state.");
        }
        return error;
    }

    return WardVzMakeError(WardVzErrorCode::UnsupportedHost,
                           @"Suspending a Virtualization.framework virtual machine requires macOS 14 or later.");
}

} // namespace

@interface WardVzMacOSVirtualMachine ()

@property (nonatomic, strong) VZVirtualMachine* virtualMachine;
@property (nonatomic, strong) WardVzMacOSConfiguration* configurationSource;
@property (nonatomic, strong) WardVzMacOSMachineState* machineState;
@property (nonatomic) dispatch_queue_t queue;
@property (nonatomic) WardVzMacOSVirtualMachineEvent event;
@property (nonatomic) void* eventContext;
@property (nonatomic, strong) NSError* saveRestoreSupportError;
@property (nonatomic) BOOL stopRequested;
@property (nonatomic) BOOL suspensionRequested;
@property (nonatomic) BOOL restorationRequested;
@property (nonatomic) BOOL invalidated;
@property (nonatomic, strong) NSHashTable<VZVirtualMachineView*>* displayViews;

- (instancetype)initWithConfiguration:(VZVirtualMachineConfiguration*)configuration
                  configurationSource:(WardVzMacOSConfiguration*)configurationSource
                         machineState:(WardVzMacOSMachineState*)machineState
                                queue:(dispatch_queue_t)queue
                                event:(WardVzMacOSVirtualMachineEvent)event
                              context:(void*)context
              saveRestoreSupportError:(nullable NSError*)saveRestoreSupportError;

- (WardVzMacOSVirtualMachineStatus)currentStatus;
- (void)emitStatusWithError:(nullable NSError*)error;
- (void)performOnQueue:(dispatch_block_t)operation;
- (void)performOnQueueSynchronously:(dispatch_block_t)operation;
- (void)requestStopOnQueue;
- (void)saveAndStopOnQueue;
- (nullable NSError*)replaceStoppedVirtualMachineOnQueue;
- (void)retargetDisplayViewsFromVirtualMachine:(VZVirtualMachine*)previousVirtualMachine
                              toVirtualMachine:(VZVirtualMachine*)replacementVirtualMachine;
- (void)finishSuspensionWithError:(nullable NSError*)error;
- (void)resumeRestoredMachineStateOnQueueWithCleanupError:(nullable NSError*)cleanupError;

@end

@implementation WardVzMacOSVirtualMachine

+ (instancetype)createWithConfiguration:(WardVzMacOSConfiguration*)configurationSource
                           machineState:(WardVzMacOSMachineState*)machineState
                                  event:(WardVzMacOSVirtualMachineEvent)event
                                context:(void*)context
                                  error:(NSError**)error
{
    VZVirtualMachineConfiguration* configuration = [configurationSource makeVirtualMachineConfigurationWithError:error];
    if (configuration == nil) {
        return nil;
    }

    NSError* saveRestoreSupportError = WardVzValidateSaveRestoreSupport(configuration);

    dispatch_queue_t queue = dispatch_queue_create("app.craftward.ward-realm-vz.vm", DISPATCH_QUEUE_SERIAL);
    return [[self alloc] initWithConfiguration:configuration
                           configurationSource:configurationSource
                                  machineState:machineState
                                         queue:queue
                                         event:event
                                       context:context
                       saveRestoreSupportError:saveRestoreSupportError];
}

- (instancetype)initWithConfiguration:(VZVirtualMachineConfiguration*)configuration
                  configurationSource:(WardVzMacOSConfiguration*)configurationSource
                         machineState:(WardVzMacOSMachineState*)machineState
                                queue:(dispatch_queue_t)queue
                                event:(WardVzMacOSVirtualMachineEvent)event
                              context:(void*)context
              saveRestoreSupportError:(NSError*)saveRestoreSupportError
{
    self = [super init];
    if (self != nil) {
        _configurationSource = configurationSource;
        _machineState = machineState;
        _queue = queue;
        _event = event;
        _eventContext = context;
        _saveRestoreSupportError = saveRestoreSupportError;
        _displayViews = [NSHashTable weakObjectsHashTable];
        dispatch_queue_set_specific(_queue, WardVzMacOSVirtualMachineQueueKey, (__bridge void*)self, nullptr);
        dispatch_sync(_queue, ^{
          self.virtualMachine = [[VZVirtualMachine alloc] initWithConfiguration:configuration queue:self.queue];
          self.virtualMachine.delegate = self;
        });
    }
    return self;
}

- (WardVzMacOSVirtualMachineStatus)status
{
    __block WardVzMacOSVirtualMachineStatus status;
    [self performOnQueueSynchronously:^{
      status = [self currentStatus];
    }];
    return status;
}

- (WardVzMacOSVirtualMachineStatus)currentStatus
{
    VZVirtualMachineState state = self.virtualMachine.state;
    WardVzMacOSVirtualMachineState reportedState = WardVzMapVirtualMachineState(state);
    BOOL hasSavedMachineState = self.machineState.hasSavedMachineState;
    BOOL lifecycleOperationRequested = self.suspensionRequested || self.restorationRequested;
    if (self.suspensionRequested) {
        reportedState = WardVzMacOSVirtualMachineStateSaving;
    } else if (self.restorationRequested) {
        reportedState = WardVzMacOSVirtualMachineStateRestoring;
    } else if (hasSavedMachineState && state == VZVirtualMachineStateStopped) {
        reportedState = WardVzMacOSVirtualMachineStateSuspended;
    } else if (self.stopRequested && state != VZVirtualMachineStateStopped && state != VZVirtualMachineStateError) {
        reportedState = WardVzMacOSVirtualMachineStateStopping;
    }

    BOOL canSuspend = self.saveRestoreSupportError == nil && !self.invalidated && !self.stopRequested &&
                      !lifecycleOperationRequested && !hasSavedMachineState &&
                      (self.virtualMachine.canPause || state == VZVirtualMachineStatePaused);
    BOOL canRestore = self.saveRestoreSupportError == nil && !self.invalidated && !lifecycleOperationRequested &&
                      hasSavedMachineState && state == VZVirtualMachineStateStopped;
    BOOL canDiscardSavedState = !self.invalidated && !lifecycleOperationRequested && hasSavedMachineState &&
                                (state == VZVirtualMachineStateStopped || state == VZVirtualMachineStateError);

    return {
        .state = reportedState,
        .can_start =
          !self.invalidated && !lifecycleOperationRequested && !hasSavedMachineState && self.virtualMachine.canStart,
        .can_pause = !self.invalidated && !self.stopRequested && !lifecycleOperationRequested &&
                     !hasSavedMachineState && self.virtualMachine.canPause,
        .can_resume = !self.invalidated && !self.stopRequested && !lifecycleOperationRequested &&
                      !hasSavedMachineState && self.virtualMachine.canResume,
        .can_request_stop = !self.invalidated && !self.stopRequested && !lifecycleOperationRequested &&
                            !hasSavedMachineState &&
                            (self.virtualMachine.canRequestStop || self.virtualMachine.canResume),
        .can_force_stop = !self.invalidated && !lifecycleOperationRequested && self.virtualMachine.canStop,
        .can_suspend = canSuspend,
        .can_restore = canRestore,
        .can_discard_saved_state = canDiscardSavedState,
    };
}

- (void)emitStatusWithError:(NSError*)error
{
    if (self.invalidated || self.event == nullptr) {
        return;
    }

    WardVzMacOSVirtualMachineStatus status = [self currentStatus];
    if (error == nil) {
        self.event(self.eventContext, &status, nullptr);
        return;
    }

    NSString* domain = error.domain != nil ? error.domain : @"app.craftward.ward-realm-vz";
    NSString* message =
      error.localizedDescription != nil ? error.localizedDescription : @"The virtual machine operation failed.";
    WardVzError bridgeError = {
        .domain = domain.UTF8String,
        .code = static_cast<int64_t>(error.code),
        .message = message.UTF8String,
    };
    self.event(self.eventContext, &status, &bridgeError);
}

- (void)performOnQueue:(dispatch_block_t)operation
{
    dispatch_async(self.queue, operation);
}

- (void)performOnQueueSynchronously:(dispatch_block_t)operation
{
    if (dispatch_get_specific(WardVzMacOSVirtualMachineQueueKey) == (__bridge void*)self) {
        operation();
    } else {
        dispatch_sync(self.queue, operation);
    }
}

- (void)start
{
    [self performOnQueue:^{
      @try {
          if (![self currentStatus].can_start) {
              [self emitStatusWithError:WardVzMakeInvalidStateError(@"start")];
              return;
          }

          self.stopRequested = NO;
          [self.virtualMachine startWithCompletionHandler:^(NSError* operationError) {
            [self emitStatusWithError:operationError];
          }];
          [self emitStatusWithError:nil];
      } @catch (NSException* exception) {
          [self emitStatusWithError:WardVzMakeBridgeExceptionError(exception)];
      }
    }];
}

- (void)pause
{
    [self performOnQueue:^{
      @try {
          if (![self currentStatus].can_pause) {
              [self emitStatusWithError:WardVzMakeInvalidStateError(@"pause")];
              return;
          }

          [self.virtualMachine pauseWithCompletionHandler:^(NSError* operationError) {
            [self emitStatusWithError:operationError];
          }];
          [self emitStatusWithError:nil];
      } @catch (NSException* exception) {
          [self emitStatusWithError:WardVzMakeBridgeExceptionError(exception)];
      }
    }];
}

- (void)resume
{
    [self performOnQueue:^{
      @try {
          if (![self currentStatus].can_resume) {
              [self emitStatusWithError:WardVzMakeInvalidStateError(@"resume")];
              return;
          }

          [self.virtualMachine resumeWithCompletionHandler:^(NSError* operationError) {
            [self emitStatusWithError:operationError];
          }];
          [self emitStatusWithError:nil];
      } @catch (NSException* exception) {
          [self emitStatusWithError:WardVzMakeBridgeExceptionError(exception)];
      }
    }];
}

- (void)requestStop
{
    [self performOnQueue:^{
      @try {
          if (![self currentStatus].can_request_stop) {
              [self emitStatusWithError:WardVzMakeInvalidStateError(@"shut down")];
              return;
          }
          if (self.virtualMachine.canRequestStop) {
              [self requestStopOnQueue];
              return;
          }
          if (!self.virtualMachine.canResume) {
              [self emitStatusWithError:WardVzMakeInvalidStateError(@"shut down")];
              return;
          }

          [self.virtualMachine resumeWithCompletionHandler:^(NSError* operationError) {
            if (operationError != nil) {
                [self emitStatusWithError:operationError];
                return;
            }
            [self requestStopOnQueue];
          }];
          [self emitStatusWithError:nil];
      } @catch (NSException* exception) {
          [self emitStatusWithError:WardVzMakeBridgeExceptionError(exception)];
      }
    }];
}

- (void)requestStopOnQueue
{
    if (!self.virtualMachine.canRequestStop) {
        [self emitStatusWithError:WardVzMakeInvalidStateError(@"shut down")];
        return;
    }

    NSError* operationError = nil;
    if (![self.virtualMachine requestStopWithError:&operationError]) {
        [self emitStatusWithError:operationError != nil ? operationError : WardVzMakeInvalidStateError(@"shut down")];
        return;
    }

    self.stopRequested = YES;
    [self emitStatusWithError:nil];
}

- (void)forceStop
{
    [self performOnQueue:^{
      @try {
          if (![self currentStatus].can_force_stop) {
              [self emitStatusWithError:WardVzMakeInvalidStateError(@"stop")];
              return;
          }

          self.stopRequested = NO;
          [self.virtualMachine stopWithCompletionHandler:^(NSError* operationError) {
            [self emitStatusWithError:operationError];
          }];
          [self emitStatusWithError:nil];
      } @catch (NSException* exception) {
          [self emitStatusWithError:WardVzMakeBridgeExceptionError(exception)];
      }
    }];
}

- (void)suspend
{
    [self performOnQueue:^{
      @try {
          WardVzMacOSVirtualMachineStatus status = [self currentStatus];
          if (!status.can_suspend) {
              NSError* error = self.saveRestoreSupportError != nil ? self.saveRestoreSupportError
                                                                   : WardVzMakeInvalidStateError(@"suspend");
              [self emitStatusWithError:error];
              return;
          }

          self.suspensionRequested = YES;
          [self emitStatusWithError:nil];
          if (self.virtualMachine.state == VZVirtualMachineStatePaused) {
              [self saveAndStopOnQueue];
              return;
          }

          [self.virtualMachine pauseWithCompletionHandler:^(NSError* operationError) {
            if (operationError != nil) {
                [self finishSuspensionWithError:operationError];
                return;
            }
            [self saveAndStopOnQueue];
          }];
      } @catch (NSException* exception) {
          [self finishSuspensionWithError:WardVzMakeBridgeExceptionError(exception)];
      }
    }];
}

- (void)saveAndStopOnQueue
{
    NSURL* savingURL = nil;
    @try {
        NSError* error = nil;
        savingURL = [self.machineState beginSavingWithError:&error];
        if (savingURL == nil) {
            [self finishSuspensionWithError:error];
            return;
        }

        if (@available(macOS 14.0, *)) {
            [self.virtualMachine
              saveMachineStateToURL:savingURL
                  completionHandler:^(NSError* operationError) {
                    @try {
                        if (operationError != nil) {
                            [self.machineState cancelSavingAtURL:savingURL];
                            [self finishSuspensionWithError:operationError];
                            return;
                        }

                        NSError* publishError = nil;
                        if (![self.machineState finishSavingAtURL:savingURL error:&publishError]) {
                            [self.machineState cancelSavingAtURL:savingURL];
                            [self finishSuspensionWithError:publishError];
                            return;
                        }
                        if (!self.virtualMachine.canStop) {
                            [self.machineState discardWithError:nil];
                            [self finishSuspensionWithError:WardVzMakeInvalidStateError(@"stop after saving")];
                            return;
                        }

                        [self.virtualMachine stopWithCompletionHandler:^(NSError* stopError) {
                          @try {
                              if (stopError != nil) {
                                  [self.machineState discardWithError:nil];
                                  [self finishSuspensionWithError:stopError];
                                  return;
                              }

                              [self finishSuspensionWithError:[self replaceStoppedVirtualMachineOnQueue]];
                          } @catch (NSException* exception) {
                              [self finishSuspensionWithError:WardVzMakeBridgeExceptionError(exception)];
                          }
                        }];
                    } @catch (NSException* exception) {
                        [self.machineState cancelSavingAtURL:savingURL];
                        [self.machineState discardWithError:nil];
                        [self finishSuspensionWithError:WardVzMakeBridgeExceptionError(exception)];
                    }
                  }];
            return;
        }

        [self.machineState cancelSavingAtURL:savingURL];
        [self finishSuspensionWithError:self.saveRestoreSupportError];
    } @catch (NSException* exception) {
        if (savingURL != nil) {
            [self.machineState cancelSavingAtURL:savingURL];
        }
        [self finishSuspensionWithError:WardVzMakeBridgeExceptionError(exception)];
    }
}

- (NSError*)replaceStoppedVirtualMachineOnQueue
{
    // Treat a successful suspension as the end of the current
    // VZVirtualMachine instance.
    //
    // A VZVirtualMachineView with automatic display reconfiguration can change
    // the runtime graphics configuration. Reusing a restored VZVirtualMachine
    // and saving it again may produce state that a fresh instance created from
    // the persistent machine configuration rejects with VZErrorRestore.
    //
    // Reconstructing the stopped machine here makes in-process and cross-process
    // restores follow the same lifecycle. Guest memory and device state remain
    // in the saved-state file; only the host-side VZ object is replaced.
    NSError* error = nil;
    VZVirtualMachineConfiguration* configuration =
      [self.configurationSource makeVirtualMachineConfigurationWithError:&error];
    if (configuration == nil) {
        return error;
    }

    error = WardVzValidateSaveRestoreSupport(configuration);
    if (error != nil) {
        return error;
    }

    VZVirtualMachine* previousVirtualMachine = self.virtualMachine;
    VZVirtualMachine* replacementVirtualMachine = [[VZVirtualMachine alloc] initWithConfiguration:configuration
                                                                                            queue:self.queue];
    replacementVirtualMachine.delegate = self;
    previousVirtualMachine.delegate = nil;
    self.virtualMachine = replacementVirtualMachine;
    self.saveRestoreSupportError = nil;
    [self retargetDisplayViewsFromVirtualMachine:previousVirtualMachine toVirtualMachine:replacementVirtualMachine];
    return nil;
}

- (void)retargetDisplayViewsFromVirtualMachine:(VZVirtualMachine*)previousVirtualMachine
                              toVirtualMachine:(VZVirtualMachine*)replacementVirtualMachine
{
    // Existing display views may outlive the retired VZVirtualMachine. Retarget
    // them on the main queue because VZVirtualMachineView is an AppKit view. The
    // weak table tracks the views without extending their lifetime.
    dispatch_async(dispatch_get_main_queue(), ^{
      for (VZVirtualMachineView* view in self.displayViews) {
          if (view.virtualMachine == previousVirtualMachine) {
              view.virtualMachine = replacementVirtualMachine;
          }
      }
    });
}

- (void)finishSuspensionWithError:(NSError*)error
{
    self.suspensionRequested = NO;
    [self emitStatusWithError:error];
}

- (void)restore
{
    [self performOnQueue:^{
      NSURL* restoringURL = nil;
      @try {
          WardVzMacOSVirtualMachineStatus status = [self currentStatus];
          if (!status.can_restore) {
              NSError* error = self.saveRestoreSupportError != nil ? self.saveRestoreSupportError
                                                                   : WardVzMakeInvalidStateError(@"restore");
              [self emitStatusWithError:error];
              return;
          }

          NSError* consumeError = nil;
          restoringURL = [self.machineState consumeWithError:&consumeError];
          if (restoringURL == nil) {
              [self emitStatusWithError:consumeError];
              return;
          }

          self.restorationRequested = YES;
          [self emitStatusWithError:nil];
          if (@available(macOS 14.0, *)) {
              [self.virtualMachine restoreMachineStateFromURL:restoringURL
                                            completionHandler:^(NSError* operationError) {
                                              NSError* cleanupError = nil;
                                              [self.machineState finishConsumingAtURL:restoringURL error:&cleanupError];
                                              if (operationError != nil) {
                                                  self.restorationRequested = NO;
                                                  [self emitStatusWithError:operationError];
                                                  return;
                                              }
                                              [self resumeRestoredMachineStateOnQueueWithCleanupError:cleanupError];
                                            }];
              return;
          }

          [self.machineState finishConsumingAtURL:restoringURL error:nil];
          self.restorationRequested = NO;
          [self emitStatusWithError:self.saveRestoreSupportError];
      } @catch (NSException* exception) {
          if (restoringURL != nil) {
              [self.machineState finishConsumingAtURL:restoringURL error:nil];
          }
          self.restorationRequested = NO;
          [self emitStatusWithError:WardVzMakeBridgeExceptionError(exception)];
      }
    }];
}

- (void)resumeRestoredMachineStateOnQueueWithCleanupError:(NSError*)cleanupError
{
    if (!self.virtualMachine.canResume) {
        self.restorationRequested = NO;
        [self emitStatusWithError:WardVzMakeInvalidStateError(@"resume after restoring")];
        return;
    }

    [self.virtualMachine resumeWithCompletionHandler:^(NSError* operationError) {
      self.restorationRequested = NO;
      [self emitStatusWithError:operationError != nil ? operationError : cleanupError];
    }];
}

- (void)discardSavedState
{
    [self performOnQueue:^{
      @try {
          if (![self currentStatus].can_discard_saved_state) {
              [self emitStatusWithError:WardVzMakeInvalidStateError(@"discard its suspended state")];
              return;
          }

          NSError* error = nil;
          [self.machineState discardWithError:&error];
          [self emitStatusWithError:error];
      } @catch (NSException* exception) {
          [self emitStatusWithError:WardVzMakeBridgeExceptionError(exception)];
      }
    }];
}

- (VZVirtualMachineView*)makeDisplayViewWithError:(NSError**)error
{
    if (!NSThread.isMainThread) {
        if (error != nullptr) {
            *error = WardVzMakeError(WardVzErrorCode::InvalidArgument,
                                     @"The virtual machine display must be created on the main thread.");
        }
        return nil;
    }

    __block VZVirtualMachine* virtualMachine = nil;
    __block BOOL invalidated = NO;
    [self performOnQueueSynchronously:^{
      invalidated = self.invalidated;
      virtualMachine = self.virtualMachine;
    }];
    if (invalidated || virtualMachine == nil) {
        if (error != nullptr) {
            *error =
              WardVzMakeError(WardVzErrorCode::InvalidState,
                              @"The virtual machine display cannot be attached after the machine is invalidated.");
        }
        return nil;
    }

    VZVirtualMachineView* view = [[VZVirtualMachineView alloc] initWithFrame:NSMakeRect(0, 0, 1280, 800)];
    view.virtualMachine = virtualMachine;
    if (@available(macOS 14.0, *)) {
        view.automaticallyReconfiguresDisplay = YES;
    }
    [self.displayViews addObject:view];
    return view;
}

- (void)invalidate
{
    [self performOnQueueSynchronously:^{
      self.invalidated = YES;
      self.event = nullptr;
      self.eventContext = nullptr;
      self.virtualMachine.delegate = nil;
    }];
}

- (void)guestDidStopVirtualMachine:(VZVirtualMachine*)virtualMachine
{
    (void)virtualMachine;
    self.stopRequested = NO;
    [self emitStatusWithError:nil];
}

- (void)virtualMachine:(VZVirtualMachine*)virtualMachine didStopWithError:(NSError*)error
{
    (void)virtualMachine;
    self.stopRequested = NO;
    [self emitStatusWithError:error];
}

@end

#endif
