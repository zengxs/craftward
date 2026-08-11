// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "macos_configuration.h"

#include "errors.h"

#include <climits>

#import <Virtualization/Virtualization.h>

#if defined(__arm64__)

namespace {

NSURL*
WardVzFileURL(const char* path, BOOL isDirectory)
{
    if (path == nullptr) {
        return nil;
    }
    return [NSURL fileURLWithFileSystemRepresentation:path isDirectory:isDirectory relativeToURL:nil];
}

NSData*
WardVzCopyBytes(WardVzByteSlice bytes)
{
    if (bytes.data == nullptr || bytes.length == 0) {
        return nil;
    }
    return [NSData dataWithBytes:bytes.data length:bytes.length];
}

} // namespace

@interface WardVzMacOSConfiguration ()

@property (nonatomic) NSUInteger CPUCount;
@property (nonatomic) uint64_t memorySize;
@property (nonatomic, copy) NSArray<NSURL*>* diskURLs;
@property (nonatomic, copy) NSURL* auxiliaryStorageURL;
@property (nonatomic, copy) NSData* hardwareModelData;
@property (nonatomic, copy) NSData* machineIdentifierData;
@property (nonatomic) NSInteger displayWidth;
@property (nonatomic) NSInteger displayHeight;
@property (nonatomic) NSInteger displayPixelsPerInch;
@property (nonatomic, strong) VZMACAddress* MACAddress;

@end

@implementation WardVzMacOSConfiguration

- (instancetype)initWithConfiguration:(const WardVzMacOSVirtualMachineConfiguration*)configuration
                                error:(NSError**)error
{
    if (configuration == nullptr || configuration->cpu_count == 0 || configuration->cpu_count > NSUIntegerMax ||
        configuration->memory_size == 0 || configuration->disk_count == 0 || configuration->disks == nullptr ||
        configuration->display_width == 0 || configuration->display_width > NSIntegerMax ||
        configuration->display_height == 0 || configuration->display_height > NSIntegerMax ||
        configuration->display_pixels_per_inch == 0 || configuration->display_pixels_per_inch > NSIntegerMax) {
        if (error != nullptr) {
            *error =
              WardVzMakeError(WardVzErrorCode::InvalidArgument, @"The macOS virtual machine configuration is invalid.");
        }
        return nil;
    }

    NSMutableArray<NSURL*>* diskURLs = [NSMutableArray arrayWithCapacity:configuration->disk_count];
    for (size_t index = 0; index < configuration->disk_count; ++index) {
        NSURL* diskURL = WardVzFileURL(configuration->disks[index].path, NO);
        if (diskURL == nil) {
            if (error != nullptr) {
                *error =
                  WardVzMakeError(WardVzErrorCode::InvalidArgument, @"A macOS virtual machine disk path is invalid.");
            }
            return nil;
        }
        [diskURLs addObject:diskURL];
    }

    NSURL* auxiliaryStorageURL = WardVzFileURL(configuration->auxiliary_storage_path, NO);
    NSData* hardwareModelData = WardVzCopyBytes(configuration->hardware_model);
    NSData* machineIdentifierData = WardVzCopyBytes(configuration->machine_identifier);
    NSString* MACAddressString =
      configuration->mac_address != nullptr ? [NSString stringWithUTF8String:configuration->mac_address] : nil;
    VZMACAddress* MACAddress = MACAddressString != nil ? [[VZMACAddress alloc] initWithString:MACAddressString] : nil;
    if (auxiliaryStorageURL == nil || hardwareModelData == nil || machineIdentifierData == nil || MACAddress == nil) {
        if (error != nullptr) {
            *error =
              WardVzMakeError(WardVzErrorCode::InvalidArgument, @"The macOS platform configuration is incomplete.");
        }
        return nil;
    }

    self = [super init];
    if (self != nil) {
        _CPUCount = static_cast<NSUInteger>(configuration->cpu_count);
        _memorySize = configuration->memory_size;
        _diskURLs = [diskURLs copy];
        _auxiliaryStorageURL = auxiliaryStorageURL;
        _hardwareModelData = hardwareModelData;
        _machineIdentifierData = machineIdentifierData;
        _displayWidth = static_cast<NSInteger>(configuration->display_width);
        _displayHeight = static_cast<NSInteger>(configuration->display_height);
        _displayPixelsPerInch = static_cast<NSInteger>(configuration->display_pixels_per_inch);
        _MACAddress = MACAddress;
    }
    return self;
}

- (VZVirtualMachineConfiguration*)makeVirtualMachineConfigurationWithError:(NSError**)outputError
{
    NSError* error = nil;
    VZMacHardwareModel* hardwareModel = [[VZMacHardwareModel alloc] initWithDataRepresentation:self.hardwareModelData];
    if (hardwareModel == nil) {
        if (outputError != nullptr) {
            *outputError =
              WardVzMakeError(WardVzErrorCode::InvalidConfiguration, @"The Mac hardware model is invalid.");
        }
        return nil;
    }

    VZMacMachineIdentifier* machineIdentifier =
      [[VZMacMachineIdentifier alloc] initWithDataRepresentation:self.machineIdentifierData];
    if (machineIdentifier == nil) {
        if (outputError != nullptr) {
            *outputError =
              WardVzMakeError(WardVzErrorCode::InvalidConfiguration, @"The Mac machine identifier is invalid.");
        }
        return nil;
    }

    if (![[NSFileManager defaultManager] fileExistsAtPath:self.auxiliaryStorageURL.path]) {
        if (outputError != nullptr) {
            *outputError =
              WardVzMakeError(WardVzErrorCode::InvalidConfiguration, @"The Mac auxiliary storage file is missing.");
        }
        return nil;
    }
    VZMacAuxiliaryStorage* auxiliaryStorage = nil;
    if (@available(macOS 13.0, *)) {
        auxiliaryStorage = [[VZMacAuxiliaryStorage alloc] initWithURL:self.auxiliaryStorageURL];
    } else {
        auxiliaryStorage = [[VZMacAuxiliaryStorage alloc] initWithContentsOfURL:self.auxiliaryStorageURL];
    }

    VZMacPlatformConfiguration* platform = [[VZMacPlatformConfiguration alloc] init];
    platform.hardwareModel = hardwareModel;
    platform.machineIdentifier = machineIdentifier;
    platform.auxiliaryStorage = auxiliaryStorage;

    NSMutableArray<VZStorageDeviceConfiguration*>* storageDevices =
      [NSMutableArray arrayWithCapacity:self.diskURLs.count];
    for (NSURL* diskURL in self.diskURLs) {
        VZDiskImageStorageDeviceAttachment* attachment =
          [[VZDiskImageStorageDeviceAttachment alloc] initWithURL:diskURL
                                                         readOnly:NO
                                                      cachingMode:VZDiskImageCachingModeAutomatic
                                              synchronizationMode:VZDiskImageSynchronizationModeFull
                                                            error:&error];
        if (attachment == nil) {
            if (outputError != nullptr) {
                *outputError = error;
            }
            return nil;
        }
        [storageDevices addObject:[[VZVirtioBlockDeviceConfiguration alloc] initWithAttachment:attachment]];
    }

    VZMacGraphicsDisplayConfiguration* display =
      [[VZMacGraphicsDisplayConfiguration alloc] initWithWidthInPixels:self.displayWidth
                                                        heightInPixels:self.displayHeight
                                                         pixelsPerInch:self.displayPixelsPerInch];
    VZMacGraphicsDeviceConfiguration* graphics = [[VZMacGraphicsDeviceConfiguration alloc] init];
    graphics.displays = @[ display ];

    VZVirtioNetworkDeviceConfiguration* network = [[VZVirtioNetworkDeviceConfiguration alloc] init];
    network.MACAddress = self.MACAddress;
    network.attachment = [[VZNATNetworkDeviceAttachment alloc] init];

    VZVirtualMachineConfiguration* configuration = [[VZVirtualMachineConfiguration alloc] init];
    configuration.bootLoader = [[VZMacOSBootLoader alloc] init];
    configuration.platform = platform;
    configuration.CPUCount = self.CPUCount;
    configuration.memorySize = self.memorySize;
    configuration.storageDevices = storageDevices;
    configuration.graphicsDevices = @[ graphics ];
    configuration.keyboards = @[ [[VZUSBKeyboardConfiguration alloc] init] ];
    configuration.pointingDevices = @[ [[VZUSBScreenCoordinatePointingDeviceConfiguration alloc] init] ];
    configuration.networkDevices = @[ network ];
    configuration.entropyDevices = @[ [[VZVirtioEntropyDeviceConfiguration alloc] init] ];

    if (![configuration validateWithError:&error]) {
        if (outputError != nullptr) {
            *outputError = error;
        }
        return nil;
    }
    return configuration;
}

@end

#endif
