// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "vz_bridge.h"

#include <cerrno>
#include <climits>
#include <fcntl.h>
#include <stdio.h>
#include <sys/stat.h>
#include <unistd.h>

#import <Foundation/Foundation.h>
#import <Virtualization/Virtualization.h>

namespace {

NSString* const WardVzErrorDomain = @"app.craftward.ward-realm-vz";

enum class WardVzErrorCode : NSInteger
{
    InvalidArgument = 1,
    UnsupportedHost = 2,
    UnsupportedRestoreImage = 3,
    DestinationExists = 4,
    BridgeException = 5,
};

NSError*
WardVzMakeError(WardVzErrorCode code, NSString* message)
{
    return [NSError errorWithDomain:WardVzErrorDomain
                               code:static_cast<NSInteger>(code)
                           userInfo:@{ NSLocalizedDescriptionKey : message }];
}

void
WardVzCompleteWithError(WardVzPrepareMacOSBundleCompletion completion, void* context, NSError* error)
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

#if defined(__arm64__)

NSError*
WardVzMakePosixError(int errorNumber)
{
    return [NSError errorWithDomain:NSPOSIXErrorDomain code:errorNumber userInfo:nil];
}

NSString* const WardVzDiskFileName = @"Disk.img";
NSString* const WardVzAuxiliaryStorageFileName = @"AuxiliaryStorage";
NSString* const WardVzHardwareModelFileName = @"HardwareModel";
NSString* const WardVzMachineIdentifierFileName = @"MachineIdentifier";
NSString* const WardVzManifestFileName = @"Manifest.json";

NSError*
WardVzRemoveTemporaryBundle(NSURL* temporaryURL, NSError* error)
{
    [[NSFileManager defaultManager] removeItemAtURL:temporaryURL error:nil];
    return error != nil ? error
                        : WardVzMakeError(WardVzErrorCode::BridgeException,
                                          @"Bundle preparation failed without an error from the host.");
}

BOOL
WardVzFailBundlePreparation(NSURL* temporaryURL, NSError* error, NSError** outputError)
{
    NSError* reportedError = WardVzRemoveTemporaryBundle(temporaryURL, error);
    if (outputError != nullptr) {
        *outputError = reportedError;
    }
    return NO;
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

NSDictionary*
WardVzCreateManifest(VZMacOSRestoreImage* restoreImage,
                     VZMacOSConfigurationRequirements* requirements,
                     uint64_t diskSize)
{
    NSOperatingSystemVersion version = restoreImage.operatingSystemVersion;
    NSString* versionString = [NSString stringWithFormat:@"%ld.%ld.%ld",
                                                         static_cast<long>(version.majorVersion),
                                                         static_cast<long>(version.minorVersion),
                                                         static_cast<long>(version.patchVersion)];

    return @{
        @"schemaVersion" : @1,
        @"backend" : @"vz",
        @"state" : @"prepared",
        @"guest" : @{
            @"family" : @"macOS",
            @"version" : versionString,
            @"buildVersion" : restoreImage.buildVersion,
        },
        @"requirements" : @{
            @"minimumCpuCount" : @(requirements.minimumSupportedCPUCount),
            @"minimumMemoryBytes" : @(requirements.minimumSupportedMemorySize),
        },
        @"files" : @{
            @"disk" : @{
                @"path" : WardVzDiskFileName,
                @"format" : @"raw",
                @"logicalSizeBytes" : @(diskSize),
            },
            @"auxiliaryStorage" : WardVzAuxiliaryStorageFileName,
            @"hardwareModel" : WardVzHardwareModelFileName,
            @"machineIdentifier" : WardVzMachineIdentifierFileName,
        },
    };
}

BOOL
WardVzPrepareBundleFiles(NSURL* destinationURL,
                         VZMacOSRestoreImage* restoreImage,
                         VZMacOSConfigurationRequirements* requirements,
                         uint64_t diskSize,
                         NSError** outputError)
{
    NSFileManager* fileManager = [NSFileManager defaultManager];
    if ([fileManager fileExistsAtPath:destinationURL.path]) {
        if (outputError != nullptr) {
            *outputError =
              WardVzMakeError(WardVzErrorCode::DestinationExists, @"The destination bundle already exists.");
        }
        return NO;
    }

    NSURL* parentURL = destinationURL.URLByDeletingLastPathComponent;
    NSError* error = nil;
    if (![fileManager createDirectoryAtURL:parentURL
               withIntermediateDirectories:YES
                                attributes:@{
                                    NSFilePosixPermissions : @0700
                                }
                                     error:&error]) {
        if (outputError != nullptr) {
            *outputError = error;
        }
        return NO;
    }

    NSString* temporaryName =
      [NSString stringWithFormat:@".%@.partial.%@", destinationURL.lastPathComponent, NSUUID.UUID.UUIDString];
    NSURL* temporaryURL = [parentURL URLByAppendingPathComponent:temporaryName isDirectory:YES];
    if (![fileManager createDirectoryAtURL:temporaryURL
               withIntermediateDirectories:NO
                                attributes:@{
                                    NSFilePosixPermissions : @0700
                                }
                                     error:&error]) {
        if (outputError != nullptr) {
            *outputError = error;
        }
        return NO;
    }

    NSURL* diskURL = [temporaryURL URLByAppendingPathComponent:WardVzDiskFileName isDirectory:NO];
    if (!WardVzCreateSparseDisk(diskURL, diskSize, &error)) {
        return WardVzFailBundlePreparation(temporaryURL, error, outputError);
    }

    VZMacHardwareModel* hardwareModel = requirements.hardwareModel;
    NSURL* auxiliaryStorageURL = [temporaryURL URLByAppendingPathComponent:WardVzAuxiliaryStorageFileName
                                                               isDirectory:NO];
    VZMacAuxiliaryStorage* auxiliaryStorage =
      [[VZMacAuxiliaryStorage alloc] initCreatingStorageAtURL:auxiliaryStorageURL
                                                hardwareModel:hardwareModel
                                                      options:0
                                                        error:&error];
    if (auxiliaryStorage == nil) {
        return WardVzFailBundlePreparation(temporaryURL, error, outputError);
    }

    NSURL* hardwareModelURL = [temporaryURL URLByAppendingPathComponent:WardVzHardwareModelFileName isDirectory:NO];
    if (![hardwareModel.dataRepresentation writeToURL:hardwareModelURL options:NSDataWritingAtomic error:&error]) {
        return WardVzFailBundlePreparation(temporaryURL, error, outputError);
    }

    VZMacMachineIdentifier* machineIdentifier = [[VZMacMachineIdentifier alloc] init];
    NSURL* machineIdentifierURL = [temporaryURL URLByAppendingPathComponent:WardVzMachineIdentifierFileName
                                                                isDirectory:NO];
    if (![machineIdentifier.dataRepresentation writeToURL:machineIdentifierURL
                                                  options:NSDataWritingAtomic
                                                    error:&error]) {
        return WardVzFailBundlePreparation(temporaryURL, error, outputError);
    }

    NSDictionary* manifest = WardVzCreateManifest(restoreImage, requirements, diskSize);
    NSData* manifestData = [NSJSONSerialization dataWithJSONObject:manifest
                                                           options:NSJSONWritingPrettyPrinted | NSJSONWritingSortedKeys
                                                             error:&error];
    NSURL* manifestURL = [temporaryURL URLByAppendingPathComponent:WardVzManifestFileName isDirectory:NO];
    if (manifestData == nil || ![manifestData writeToURL:manifestURL options:NSDataWritingAtomic error:&error]) {
        return WardVzFailBundlePreparation(temporaryURL, error, outputError);
    }

    if (renamex_np(temporaryURL.fileSystemRepresentation, destinationURL.fileSystemRepresentation, RENAME_EXCL) != 0) {
        return WardVzFailBundlePreparation(temporaryURL, WardVzMakePosixError(errno), outputError);
    }

    return YES;
}

void
WardVzStartPreparingMacOSBundle(NSURL* restoreImageURL,
                                NSURL* destinationURL,
                                uint64_t diskSize,
                                WardVzPrepareMacOSBundleCompletion completion,
                                void* context)
{
    @try {
        [VZMacOSRestoreImage
                loadFileURL:restoreImageURL
          completionHandler:^(VZMacOSRestoreImage* restoreImage, NSError* error) {
            @autoreleasepool {
                @try {
                    if (error != nil) {
                        WardVzCompleteWithError(completion, context, error);
                        return;
                    }
                    if (restoreImage == nil) {
                        WardVzCompleteWithError(
                          completion,
                          context,
                          WardVzMakeError(WardVzErrorCode::BridgeException,
                                          @"Virtualization.framework returned no restore image."));
                        return;
                    }

                    VZMacOSConfigurationRequirements* requirements = restoreImage.mostFeaturefulSupportedConfiguration;
                    if (requirements == nil) {
                        WardVzCompleteWithError(
                          completion,
                          context,
                          WardVzMakeError(WardVzErrorCode::UnsupportedRestoreImage,
                                          @"The restore image has no configuration supported by this host."));
                        return;
                    }

                    NSError* preparationError = nil;
                    if (!WardVzPrepareBundleFiles(
                          destinationURL, restoreImage, requirements, diskSize, &preparationError)) {
                        WardVzCompleteWithError(completion, context, preparationError);
                        return;
                    }

                    NSOperatingSystemVersion version = restoreImage.operatingSystemVersion;
                    WardVzMacOSBundleInfo bundleInfo = {
                        .build_version = restoreImage.buildVersion.UTF8String,
                        .os_version_major = static_cast<uint64_t>(version.majorVersion),
                        .os_version_minor = static_cast<uint64_t>(version.minorVersion),
                        .os_version_patch = static_cast<uint64_t>(version.patchVersion),
                        .minimum_cpu_count = static_cast<uint64_t>(requirements.minimumSupportedCPUCount),
                        .minimum_memory_size = requirements.minimumSupportedMemorySize,
                        .disk_size = diskSize,
                    };
                    completion(context, &bundleInfo, nullptr);
                } @catch (NSException* exception) {
                    NSString* message = exception.reason != nil ? exception.reason : exception.name;
                    WardVzCompleteWithError(
                      completion, context, WardVzMakeError(WardVzErrorCode::BridgeException, message));
                }
            }
          }];
    } @catch (NSException* exception) {
        NSString* message = exception.reason != nil ? exception.reason : exception.name;
        WardVzCompleteWithError(completion, context, WardVzMakeError(WardVzErrorCode::BridgeException, message));
    }
}

#endif

} // namespace

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
                WardVzCompleteWithError(completion,
                                        context,
                                        WardVzMakeError(WardVzErrorCode::UnsupportedHost,
                                                        @"Virtualization.framework is unavailable on this host."));
                return;
            }
            if (restoreImagePath == nullptr || destinationPath == nullptr || diskSize == 0 ||
                diskSize > static_cast<uint64_t>(LLONG_MAX)) {
                WardVzCompleteWithError(
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
                WardVzCompleteWithError(
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
        WardVzCompleteWithError(completion,
                                context,
                                WardVzMakeError(WardVzErrorCode::UnsupportedHost,
                                                @"This host cannot create a Virtualization.framework macOS guest."));
    }
}
