// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "macos_bundle.h"

#include "errors.h"

#include <cerrno>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

#import <Virtualization/Virtualization.h>

#if defined(__arm64__)

namespace {

NSString* const WardVzDiskFileName = @"Disk.img";
NSString* const WardVzAuxiliaryStorageFileName = @"AuxiliaryStorage";
NSString* const WardVzHardwareModelFileName = @"HardwareModel";
NSString* const WardVzMachineIdentifierFileName = @"MachineIdentifier";
NSString* const WardVzManifestFileName = @"Manifest.json";
NSString* const WardVzPreparedState = @"prepared";
NSString* const WardVzInstallingState = @"installing";
NSString* const WardVzInstalledState = @"installed";
NSString* const WardVzInstallationFailedState = @"installationFailed";

constexpr NSInteger WardVzDisplayWidth = 1920;
constexpr NSInteger WardVzDisplayHeight = 1200;
constexpr NSInteger WardVzDisplayPixelsPerInch = 80;

NSError*
WardVzMakePosixError(int errorNumber)
{
    return [NSError errorWithDomain:NSPOSIXErrorDomain code:errorNumber userInfo:nil];
}

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
    VZMACAddress* MACAddress = VZMACAddress.randomLocallyAdministeredAddress;

    return @{
        @"schemaVersion" : @1,
        @"backend" : @"vz",
        @"state" : WardVzPreparedState,
        @"guest" : @{
            @"family" : @"macOS",
            @"version" : versionString,
            @"buildVersion" : restoreImage.buildVersion,
        },
        @"requirements" : @{
            @"minimumCpuCount" : @(requirements.minimumSupportedCPUCount),
            @"minimumMemoryBytes" : @(requirements.minimumSupportedMemorySize),
        },
        @"configuration" : @{
            @"cpuCount" : @(requirements.minimumSupportedCPUCount),
            @"memoryBytes" : @(requirements.minimumSupportedMemorySize),
            @"display" : @{
                @"widthPixels" : @(WardVzDisplayWidth),
                @"heightPixels" : @(WardVzDisplayHeight),
                @"pixelsPerInch" : @(WardVzDisplayPixelsPerInch),
            },
            @"network" : @{
                @"macAddress" : MACAddress.string,
            },
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

NSMutableDictionary*
WardVzLoadBundleManifest(NSURL* bundleURL,
                         NSString* expectedState,
                         NSString* invalidStateDescription,
                         NSError** outputError)
{
    NSURL* manifestURL = [bundleURL URLByAppendingPathComponent:WardVzManifestFileName isDirectory:NO];
    NSData* data = [NSData dataWithContentsOfURL:manifestURL options:0 error:outputError];
    if (data == nil) {
        return nil;
    }

    NSError* error = nil;
    id value = [NSJSONSerialization JSONObjectWithData:data options:0 error:&error];
    if (![value isKindOfClass:NSDictionary.class]) {
        if (outputError != nullptr) {
            *outputError = error != nil ? error
                                        : WardVzMakeError(WardVzErrorCode::InvalidBundle,
                                                          @"The bundle manifest is not a JSON object.");
        }
        return nil;
    }

    NSMutableDictionary* manifest = [value mutableCopy];
    NSNumber* schemaVersion = manifest[@"schemaVersion"];
    NSString* backend = manifest[@"backend"];
    NSDictionary* guest = manifest[@"guest"];
    NSString* family = [guest isKindOfClass:NSDictionary.class] ? guest[@"family"] : nil;
    if (![schemaVersion isKindOfClass:NSNumber.class] || schemaVersion.unsignedIntegerValue != 1 ||
        ![backend isKindOfClass:NSString.class] || ![backend isEqualToString:@"vz"] ||
        ![family isKindOfClass:NSString.class] || ![family isEqualToString:@"macOS"]) {
        if (outputError != nullptr) {
            *outputError = WardVzMakeError(WardVzErrorCode::InvalidBundle,
                                           @"The bundle manifest is not a supported VZ macOS manifest.");
        }
        return nil;
    }

    id state = manifest[@"state"];
    BOOL allowsLegacyPreparedState = state == nil && [expectedState isEqualToString:WardVzPreparedState];
    if (!allowsLegacyPreparedState &&
        (![state isKindOfClass:NSString.class] || ![state isEqualToString:expectedState])) {
        if (outputError != nullptr) {
            NSString* message = [NSString stringWithFormat:@"The bundle cannot be %@ from its current state (%@).",
                                                           invalidStateDescription,
                                                           state != nil ? state : @"missing"];
            *outputError = WardVzMakeError(WardVzErrorCode::InvalidBundleState, message);
        }
        return nil;
    }

    return manifest;
}

BOOL
WardVzWriteBundleManifest(NSURL* bundleURL, NSDictionary* manifest, NSError** outputError)
{
    NSError* error = nil;
    NSData* data = [NSJSONSerialization dataWithJSONObject:manifest
                                                   options:NSJSONWritingPrettyPrinted | NSJSONWritingSortedKeys
                                                     error:&error];
    if (data == nil) {
        if (outputError != nullptr) {
            *outputError = error;
        }
        return NO;
    }

    NSURL* manifestURL = [bundleURL URLByAppendingPathComponent:WardVzManifestFileName isDirectory:NO];
    return [data writeToURL:manifestURL options:NSDataWritingAtomic error:outputError];
}

NSDictionary*
WardVzResolveConfigurationManifest(NSMutableDictionary* manifest, NSError** outputError)
{
    id value = manifest[@"configuration"];
    if (value == nil) {
        NSDictionary* requirements = manifest[@"requirements"];
        NSNumber* CPUCount = [requirements isKindOfClass:NSDictionary.class] ? requirements[@"minimumCpuCount"] : nil;
        NSNumber* memorySize =
          [requirements isKindOfClass:NSDictionary.class] ? requirements[@"minimumMemoryBytes"] : nil;
        if (![CPUCount isKindOfClass:NSNumber.class] || ![memorySize isKindOfClass:NSNumber.class]) {
            if (outputError != nullptr) {
                *outputError = WardVzMakeError(WardVzErrorCode::InvalidBundle,
                                               @"The bundle manifest has no usable resource requirements.");
            }
            return nil;
        }

        value = @{
            @"cpuCount" : CPUCount,
            @"memoryBytes" : memorySize,
            @"display" : @{
                @"widthPixels" : @(WardVzDisplayWidth),
                @"heightPixels" : @(WardVzDisplayHeight),
                @"pixelsPerInch" : @(WardVzDisplayPixelsPerInch),
            },
            @"network" : @{
                @"macAddress" : VZMACAddress.randomLocallyAdministeredAddress.string,
            },
        };
        manifest[@"configuration"] = value;
    }

    if (![value isKindOfClass:NSDictionary.class]) {
        if (outputError != nullptr) {
            *outputError =
              WardVzMakeError(WardVzErrorCode::InvalidBundle, @"The bundle configuration is not a JSON object.");
        }
        return nil;
    }
    return value;
}

BOOL
WardVzReadPositiveNumber(NSDictionary* dictionary, NSString* key, uint64_t* outputValue, NSError** outputError)
{
    NSNumber* number = dictionary[key];
    uint64_t value = [number isKindOfClass:NSNumber.class] ? number.unsignedLongLongValue : 0;
    if (value == 0) {
        if (outputError != nullptr) {
            NSString* message = [NSString stringWithFormat:@"The bundle configuration has an invalid %@.", key];
            *outputError = WardVzMakeError(WardVzErrorCode::InvalidBundle, message);
        }
        return NO;
    }
    *outputValue = value;
    return YES;
}

VZVirtualMachineConfiguration*
WardVzCreateMacOSVirtualMachineConfiguration(NSURL* bundleURL, NSMutableDictionary* manifest, NSError** outputError)
{
    NSDictionary* storedConfiguration = WardVzResolveConfigurationManifest(manifest, outputError);
    if (storedConfiguration == nil) {
        return nil;
    }

    uint64_t CPUCount = 0;
    uint64_t memorySize = 0;
    if (!WardVzReadPositiveNumber(storedConfiguration, @"cpuCount", &CPUCount, outputError) ||
        !WardVzReadPositiveNumber(storedConfiguration, @"memoryBytes", &memorySize, outputError)) {
        return nil;
    }

    NSDictionary* display = storedConfiguration[@"display"];
    uint64_t displayWidth = 0;
    uint64_t displayHeight = 0;
    uint64_t displayPixelsPerInch = 0;
    if (![display isKindOfClass:NSDictionary.class] ||
        !WardVzReadPositiveNumber(display, @"widthPixels", &displayWidth, outputError) ||
        !WardVzReadPositiveNumber(display, @"heightPixels", &displayHeight, outputError) ||
        !WardVzReadPositiveNumber(display, @"pixelsPerInch", &displayPixelsPerInch, outputError)) {
        return nil;
    }

    NSDictionary* network = storedConfiguration[@"network"];
    NSString* MACAddressString = [network isKindOfClass:NSDictionary.class] ? network[@"macAddress"] : nil;
    VZMACAddress* MACAddress =
      [MACAddressString isKindOfClass:NSString.class] ? [[VZMACAddress alloc] initWithString:MACAddressString] : nil;
    if (MACAddress == nil || CPUCount > NSUIntegerMax || displayWidth > NSIntegerMax || displayHeight > NSIntegerMax ||
        displayPixelsPerInch > NSIntegerMax) {
        if (outputError != nullptr) {
            *outputError = WardVzMakeError(WardVzErrorCode::InvalidBundle,
                                           @"The bundle configuration contains an unsupported value.");
        }
        return nil;
    }

    NSError* error = nil;
    NSURL* hardwareModelURL = [bundleURL URLByAppendingPathComponent:WardVzHardwareModelFileName isDirectory:NO];
    NSData* hardwareModelData = [NSData dataWithContentsOfURL:hardwareModelURL options:0 error:&error];
    VZMacHardwareModel* hardwareModel =
      hardwareModelData != nil ? [[VZMacHardwareModel alloc] initWithDataRepresentation:hardwareModelData] : nil;
    if (hardwareModel == nil) {
        if (outputError != nullptr) {
            *outputError = error != nil ? error
                                        : WardVzMakeError(WardVzErrorCode::InvalidBundle,
                                                          @"The bundle contains an invalid Mac hardware model.");
        }
        return nil;
    }

    NSURL* machineIdentifierURL = [bundleURL URLByAppendingPathComponent:WardVzMachineIdentifierFileName
                                                             isDirectory:NO];
    NSData* machineIdentifierData = [NSData dataWithContentsOfURL:machineIdentifierURL options:0 error:&error];
    VZMacMachineIdentifier* machineIdentifier =
      machineIdentifierData != nil ? [[VZMacMachineIdentifier alloc] initWithDataRepresentation:machineIdentifierData]
                                   : nil;
    if (machineIdentifier == nil) {
        if (outputError != nullptr) {
            *outputError = error != nil ? error
                                        : WardVzMakeError(WardVzErrorCode::InvalidBundle,
                                                          @"The bundle contains an invalid Mac machine identifier.");
        }
        return nil;
    }

    NSURL* auxiliaryStorageURL = [bundleURL URLByAppendingPathComponent:WardVzAuxiliaryStorageFileName isDirectory:NO];
    if (![[NSFileManager defaultManager] fileExistsAtPath:auxiliaryStorageURL.path]) {
        if (outputError != nullptr) {
            *outputError =
              WardVzMakeError(WardVzErrorCode::InvalidBundle, @"The bundle has no auxiliary storage file.");
        }
        return nil;
    }
    VZMacAuxiliaryStorage* auxiliaryStorage = nil;
    if (@available(macOS 13.0, *)) {
        auxiliaryStorage = [[VZMacAuxiliaryStorage alloc] initWithURL:auxiliaryStorageURL];
    } else {
        auxiliaryStorage = [[VZMacAuxiliaryStorage alloc] initWithContentsOfURL:auxiliaryStorageURL];
    }

    VZMacPlatformConfiguration* platform = [[VZMacPlatformConfiguration alloc] init];
    platform.hardwareModel = hardwareModel;
    platform.machineIdentifier = machineIdentifier;
    platform.auxiliaryStorage = auxiliaryStorage;

    NSURL* diskURL = [bundleURL URLByAppendingPathComponent:WardVzDiskFileName isDirectory:NO];
    VZDiskImageStorageDeviceAttachment* diskAttachment =
      [[VZDiskImageStorageDeviceAttachment alloc] initWithURL:diskURL
                                                     readOnly:NO
                                                  cachingMode:VZDiskImageCachingModeAutomatic
                                          synchronizationMode:VZDiskImageSynchronizationModeFull
                                                        error:&error];
    if (diskAttachment == nil) {
        if (outputError != nullptr) {
            *outputError = error;
        }
        return nil;
    }
    VZVirtioBlockDeviceConfiguration* blockDevice =
      [[VZVirtioBlockDeviceConfiguration alloc] initWithAttachment:diskAttachment];

    VZMacGraphicsDisplayConfiguration* displayConfiguration =
      [[VZMacGraphicsDisplayConfiguration alloc] initWithWidthInPixels:static_cast<NSInteger>(displayWidth)
                                                        heightInPixels:static_cast<NSInteger>(displayHeight)
                                                         pixelsPerInch:static_cast<NSInteger>(displayPixelsPerInch)];
    VZMacGraphicsDeviceConfiguration* graphicsDevice = [[VZMacGraphicsDeviceConfiguration alloc] init];
    graphicsDevice.displays = @[ displayConfiguration ];

    VZVirtioNetworkDeviceConfiguration* networkDevice = [[VZVirtioNetworkDeviceConfiguration alloc] init];
    networkDevice.MACAddress = MACAddress;
    networkDevice.attachment = [[VZNATNetworkDeviceAttachment alloc] init];

    VZVirtualMachineConfiguration* configuration = [[VZVirtualMachineConfiguration alloc] init];
    configuration.bootLoader = [[VZMacOSBootLoader alloc] init];
    configuration.platform = platform;
    configuration.CPUCount = static_cast<NSUInteger>(CPUCount);
    configuration.memorySize = memorySize;
    configuration.storageDevices = @[ blockDevice ];
    configuration.graphicsDevices = @[ graphicsDevice ];
    configuration.keyboards = @[ [[VZUSBKeyboardConfiguration alloc] init] ];
    configuration.pointingDevices = @[ [[VZUSBScreenCoordinatePointingDeviceConfiguration alloc] init] ];
    configuration.networkDevices = @[ networkDevice ];
    configuration.entropyDevices = @[ [[VZVirtioEntropyDeviceConfiguration alloc] init] ];

    if (![configuration validateWithError:&error]) {
        if (outputError != nullptr) {
            *outputError = error;
        }
        return nil;
    }
    return configuration;
}

} // namespace

@interface WardVzMacOSBundle ()

@property (nonatomic, copy) NSURL* bundleURL;
@property (nonatomic, strong) NSMutableDictionary* manifest;

- (instancetype)initWithURL:(NSURL*)URL manifest:(NSMutableDictionary*)manifest;

@end

@implementation WardVzMacOSBundle

+ (instancetype)openPreparedBundleAtURL:(NSURL*)URL error:(NSError**)error
{
    NSMutableDictionary* manifest = WardVzLoadBundleManifest(URL, WardVzPreparedState, @"installed", error);
    if (manifest == nil) {
        return nil;
    }
    return [[self alloc] initWithURL:URL manifest:manifest];
}

+ (instancetype)openInstalledBundleAtURL:(NSURL*)URL error:(NSError**)error
{
    NSMutableDictionary* manifest = WardVzLoadBundleManifest(URL, WardVzInstalledState, @"started", error);
    if (manifest == nil) {
        return nil;
    }
    return [[self alloc] initWithURL:URL manifest:manifest];
}

- (instancetype)initWithURL:(NSURL*)URL manifest:(NSMutableDictionary*)manifest
{
    self = [super init];
    if (self != nil) {
        _bundleURL = [URL copy];
        _manifest = manifest;
    }
    return self;
}

- (VZVirtualMachineConfiguration*)createVirtualMachineConfigurationWithError:(NSError**)error
{
    return WardVzCreateMacOSVirtualMachineConfiguration(self.bundleURL, self.manifest, error);
}

- (BOOL)transitionToInstallingWithError:(NSError**)error
{
    self.manifest[@"state"] = WardVzInstallingState;
    return WardVzWriteBundleManifest(self.bundleURL, self.manifest, error);
}

- (BOOL)transitionToInstalledWithError:(NSError**)error
{
    self.manifest[@"state"] = WardVzInstalledState;
    return WardVzWriteBundleManifest(self.bundleURL, self.manifest, error);
}

- (void)transitionToInstallationFailed
{
    self.manifest[@"state"] = WardVzInstallationFailedState;
    WardVzWriteBundleManifest(self.bundleURL, self.manifest, nil);
}

@end

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
                        WardVzCompleteBundlePreparationWithError(completion, context, error);
                        return;
                    }
                    if (restoreImage == nil) {
                        WardVzCompleteBundlePreparationWithError(
                          completion,
                          context,
                          WardVzMakeError(WardVzErrorCode::BridgeException,
                                          @"Virtualization.framework returned no restore image."));
                        return;
                    }

                    VZMacOSConfigurationRequirements* requirements = restoreImage.mostFeaturefulSupportedConfiguration;
                    if (requirements == nil) {
                        WardVzCompleteBundlePreparationWithError(
                          completion,
                          context,
                          WardVzMakeError(WardVzErrorCode::UnsupportedRestoreImage,
                                          @"The restore image has no configuration supported by this host."));
                        return;
                    }

                    NSError* preparationError = nil;
                    if (!WardVzPrepareBundleFiles(
                          destinationURL, restoreImage, requirements, diskSize, &preparationError)) {
                        WardVzCompleteBundlePreparationWithError(completion, context, preparationError);
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
                    WardVzCompleteBundlePreparationWithError(
                      completion, context, WardVzMakeError(WardVzErrorCode::BridgeException, message));
                }
            }
          }];
    } @catch (NSException* exception) {
        NSString* message = exception.reason != nil ? exception.reason : exception.name;
        WardVzCompleteBundlePreparationWithError(
          completion, context, WardVzMakeError(WardVzErrorCode::BridgeException, message));
    }
}

#endif
