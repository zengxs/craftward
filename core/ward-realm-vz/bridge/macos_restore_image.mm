// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "macos_restore_image.h"

#include "errors.h"

#include <cerrno>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

#import <Virtualization/Virtualization.h>

#if defined(__arm64__)

namespace {

NSError*
WardVzMakePosixError(int errorNumber)
{
    return [NSError errorWithDomain:NSPOSIXErrorDomain code:errorNumber userInfo:nil];
}

BOOL
WardVzCreateSparseDisk(NSURL* URL, uint64_t diskSize, NSError** error)
{
    int descriptor = open(URL.fileSystemRepresentation, O_CREAT | O_EXCL | O_WRONLY, S_IRUSR | S_IWUSR);
    if (descriptor < 0) {
        if (error != nullptr) {
            *error = WardVzMakePosixError(errno);
        }
        return NO;
    }

    if (ftruncate(descriptor, static_cast<off_t>(diskSize)) != 0) {
        int errorNumber = errno;
        close(descriptor);
        if (error != nullptr) {
            *error = WardVzMakePosixError(errorNumber);
        }
        return NO;
    }
    if (close(descriptor) != 0) {
        if (error != nullptr) {
            *error = WardVzMakePosixError(errno);
        }
        return NO;
    }
    return YES;
}

} // namespace

void
WardVzStartPreparingMacOS(NSURL* restoreImageURL,
                          NSURL* diskURL,
                          NSURL* auxiliaryStorageURL,
                          uint64_t diskSize,
                          WardVzPrepareMacOSCompletion completion,
                          void* context)
{
    @try {
        [VZMacOSRestoreImage
                loadFileURL:restoreImageURL
          completionHandler:^(VZMacOSRestoreImage* restoreImage, NSError* error) {
            @autoreleasepool {
                @try {
                    if (error != nil) {
                        WardVzCompleteMacOSPreparationWithError(completion, context, error);
                        return;
                    }
                    if (restoreImage == nil) {
                        WardVzCompleteMacOSPreparationWithError(
                          completion,
                          context,
                          WardVzMakeError(WardVzErrorCode::BridgeException,
                                          @"Virtualization.framework returned no restore image."));
                        return;
                    }

                    VZMacOSConfigurationRequirements* requirements = restoreImage.mostFeaturefulSupportedConfiguration;
                    if (requirements == nil) {
                        WardVzCompleteMacOSPreparationWithError(
                          completion,
                          context,
                          WardVzMakeError(WardVzErrorCode::UnsupportedRestoreImage,
                                          @"The restore image has no configuration supported by this host."));
                        return;
                    }

                    NSError* preparationError = nil;
                    if (!WardVzCreateSparseDisk(diskURL, diskSize, &preparationError)) {
                        WardVzCompleteMacOSPreparationWithError(completion, context, preparationError);
                        return;
                    }

                    VZMacHardwareModel* hardwareModel = requirements.hardwareModel;
                    VZMacAuxiliaryStorage* auxiliaryStorage =
                      [[VZMacAuxiliaryStorage alloc] initCreatingStorageAtURL:auxiliaryStorageURL
                                                                hardwareModel:hardwareModel
                                                                      options:0
                                                                        error:&preparationError];
                    if (auxiliaryStorage == nil) {
                        WardVzCompleteMacOSPreparationWithError(completion, context, preparationError);
                        return;
                    }

                    VZMacMachineIdentifier* machineIdentifier = [[VZMacMachineIdentifier alloc] init];
                    VZMACAddress* MACAddress = VZMACAddress.randomLocallyAdministeredAddress;
                    NSData* hardwareModelData = hardwareModel.dataRepresentation;
                    NSData* machineIdentifierData = machineIdentifier.dataRepresentation;
                    NSOperatingSystemVersion version = restoreImage.operatingSystemVersion;
                    WardVzMacOSPreparationInfo info = {
                        .build_version = restoreImage.buildVersion.UTF8String,
                        .os_version_major = static_cast<uint64_t>(version.majorVersion),
                        .os_version_minor = static_cast<uint64_t>(version.minorVersion),
                        .os_version_patch = static_cast<uint64_t>(version.patchVersion),
                        .minimum_cpu_count = static_cast<uint64_t>(requirements.minimumSupportedCPUCount),
                        .minimum_memory_size = requirements.minimumSupportedMemorySize,
                        .hardware_model = {
                          .data = static_cast<const uint8_t*>(hardwareModelData.bytes),
                          .length = hardwareModelData.length,
                        },
                        .machine_identifier = {
                          .data = static_cast<const uint8_t*>(machineIdentifierData.bytes),
                          .length = machineIdentifierData.length,
                        },
                        .mac_address = MACAddress.string.UTF8String,
                    };
                    completion(context, &info, nullptr);
                } @catch (NSException* exception) {
                    NSString* message = exception.reason != nil ? exception.reason : exception.name;
                    WardVzCompleteMacOSPreparationWithError(
                      completion, context, WardVzMakeError(WardVzErrorCode::BridgeException, message));
                }
            }
          }];
    } @catch (NSException* exception) {
        NSString* message = exception.reason != nil ? exception.reason : exception.name;
        WardVzCompleteMacOSPreparationWithError(
          completion, context, WardVzMakeError(WardVzErrorCode::BridgeException, message));
    }
}

#endif
