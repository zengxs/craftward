// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "macos_vm.h"

#include "errors.h"
#include "macos_bundle.h"

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
}

NSError*
WardVzMakeInvalidStateError(NSString* operation)
{
    return WardVzMakeError(
      WardVzErrorCode::InvalidBundleState,
      [NSString stringWithFormat:@"The virtual machine cannot %@ from its current state.", operation]);
}

NSError*
WardVzMakeBridgeExceptionError(NSException* exception)
{
    NSString* message = exception.reason != nil ? exception.reason : exception.name;
    return WardVzMakeError(WardVzErrorCode::BridgeException, message);
}

} // namespace

@interface WardVzMacOSVirtualMachine ()

@property (nonatomic, strong) VZVirtualMachine* virtualMachine;
@property (nonatomic) dispatch_queue_t queue;
@property (nonatomic) WardVzMacOSVirtualMachineEvent event;
@property (nonatomic) void* eventContext;
@property (nonatomic) BOOL stopRequested;
@property (nonatomic) BOOL invalidated;

- (instancetype)initWithConfiguration:(VZVirtualMachineConfiguration*)configuration
                                queue:(dispatch_queue_t)queue
                                event:(WardVzMacOSVirtualMachineEvent)event
                              context:(void*)context;

- (WardVzMacOSVirtualMachineStatus)currentStatus;
- (void)emitStatusWithError:(nullable NSError*)error;
- (void)performOnQueue:(dispatch_block_t)operation;
- (void)performOnQueueSynchronously:(dispatch_block_t)operation;
- (void)requestStopOnQueue;

@end

@implementation WardVzMacOSVirtualMachine

+ (instancetype)openInstalledBundleAtURL:(NSURL*)bundleURL
                                   event:(WardVzMacOSVirtualMachineEvent)event
                                 context:(void*)context
                                   error:(NSError**)error
{
    WardVzMacOSBundle* bundle = [WardVzMacOSBundle openInstalledBundleAtURL:bundleURL error:error];
    if (bundle == nil) {
        return nil;
    }

    VZVirtualMachineConfiguration* configuration = [bundle createVirtualMachineConfigurationWithError:error];
    if (configuration == nil) {
        return nil;
    }

    dispatch_queue_t queue = dispatch_queue_create("app.craftward.ward-realm-vz.vm", DISPATCH_QUEUE_SERIAL);
    return [[self alloc] initWithConfiguration:configuration queue:queue event:event context:context];
}

- (instancetype)initWithConfiguration:(VZVirtualMachineConfiguration*)configuration
                                queue:(dispatch_queue_t)queue
                                event:(WardVzMacOSVirtualMachineEvent)event
                              context:(void*)context
{
    self = [super init];
    if (self != nil) {
        _queue = queue;
        _event = event;
        _eventContext = context;
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
    if (self.stopRequested && state != VZVirtualMachineStateStopped && state != VZVirtualMachineStateError) {
        reportedState = WardVzMacOSVirtualMachineStateStopping;
    }

    return {
        .state = reportedState,
        .can_start = !self.invalidated && self.virtualMachine.canStart,
        .can_pause = !self.invalidated && !self.stopRequested && self.virtualMachine.canPause,
        .can_resume = !self.invalidated && !self.stopRequested && self.virtualMachine.canResume,
        .can_request_stop = !self.invalidated && !self.stopRequested &&
                            (self.virtualMachine.canRequestStop || self.virtualMachine.canResume),
        .can_force_stop = !self.invalidated && self.virtualMachine.canStop,
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
          if (!self.virtualMachine.canStart) {
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
          if (!self.virtualMachine.canPause) {
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
          if (!self.virtualMachine.canResume) {
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
          if (!self.virtualMachine.canStop) {
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
              WardVzMakeError(WardVzErrorCode::InvalidBundleState,
                              @"The virtual machine display cannot be attached after the machine is invalidated.");
        }
        return nil;
    }

    VZVirtualMachineView* view = [[VZVirtualMachineView alloc] initWithFrame:NSMakeRect(0, 0, 1280, 800)];
    view.virtualMachine = virtualMachine;
    if (@available(macOS 14.0, *)) {
        view.automaticallyReconfiguresDisplay = YES;
    }
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
