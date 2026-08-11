// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C"
{
#endif

    bool ward_vz_is_supported(void);

    typedef struct WardVzByteSlice
    {
        const uint8_t* data;
        size_t length;
    } WardVzByteSlice;

    typedef struct WardVzMacOSPreparationInfo
    {
        const char* build_version;
        uint64_t os_version_major;
        uint64_t os_version_minor;
        uint64_t os_version_patch;
        uint64_t minimum_cpu_count;
        uint64_t minimum_memory_size;
        WardVzByteSlice hardware_model;
        WardVzByteSlice machine_identifier;
        const char* mac_address;
    } WardVzMacOSPreparationInfo;

    typedef struct WardVzMacOSDiskConfiguration
    {
        const char* path;
    } WardVzMacOSDiskConfiguration;

    typedef struct WardVzMacOSVirtualMachineConfiguration
    {
        uint64_t cpu_count;
        uint64_t memory_size;
        const WardVzMacOSDiskConfiguration* disks;
        size_t disk_count;
        const char* auxiliary_storage_path;
        WardVzByteSlice hardware_model;
        WardVzByteSlice machine_identifier;
        uint64_t display_width;
        uint64_t display_height;
        uint64_t display_pixels_per_inch;
        const char* mac_address;
    } WardVzMacOSVirtualMachineConfiguration;

    typedef struct WardVzError
    {
        const char* domain;
        int64_t code;
        const char* message;
    } WardVzError;

    typedef void (*WardVzPrepareMacOSCompletion)(void* context,
                                                 const WardVzMacOSPreparationInfo* preparation_info,
                                                 const WardVzError* error);

    typedef void (*WardVzMacOSInstallationProgress)(void* context, double fraction_completed);

    typedef void (*WardVzInstallMacOSCompletion)(void* context, const WardVzError* error);

    typedef enum WardVzMacOSVirtualMachineState
    {
        WardVzMacOSVirtualMachineStateStopped = 0,
        WardVzMacOSVirtualMachineStateRunning = 1,
        WardVzMacOSVirtualMachineStatePaused = 2,
        WardVzMacOSVirtualMachineStateError = 3,
        WardVzMacOSVirtualMachineStateStarting = 4,
        WardVzMacOSVirtualMachineStatePausing = 5,
        WardVzMacOSVirtualMachineStateResuming = 6,
        WardVzMacOSVirtualMachineStateStopping = 7,
        WardVzMacOSVirtualMachineStateSaving = 8,
        WardVzMacOSVirtualMachineStateRestoring = 9,
        WardVzMacOSVirtualMachineStateSuspended = 10,
    } WardVzMacOSVirtualMachineState;

    typedef struct WardVzMacOSVirtualMachineStatus
    {
        WardVzMacOSVirtualMachineState state;
        bool can_start;
        bool can_pause;
        bool can_resume;
        bool can_request_stop;
        bool can_force_stop;
        bool can_suspend;
        bool can_restore;
        bool can_discard_saved_state;
    } WardVzMacOSVirtualMachineStatus;

    typedef struct WardVzMacOSVirtualMachineHandle WardVzMacOSVirtualMachineHandle;
    typedef struct WardVzMacOSVirtualMachineDisplayHandle WardVzMacOSVirtualMachineDisplayHandle;

    typedef void (*WardVzMacOSVirtualMachineEvent)(void* context,
                                                   const WardVzMacOSVirtualMachineStatus* status,
                                                   const WardVzError* error);

    typedef void (*WardVzCreateMacOSVirtualMachineCompletion)(void* context,
                                                              WardVzMacOSVirtualMachineHandle* virtual_machine,
                                                              const WardVzMacOSVirtualMachineStatus* status,
                                                              const WardVzError* error);

    typedef void (*WardVzCreateMacOSVirtualMachineDisplayCompletion)(void* context,
                                                                     WardVzMacOSVirtualMachineDisplayHandle* display,
                                                                     void* native_view,
                                                                     const WardVzError* error);

    void ward_vz_prepare_macos(const char* restore_image_path,
                               const char* disk_path,
                               const char* auxiliary_storage_path,
                               uint64_t disk_size,
                               WardVzPrepareMacOSCompletion completion,
                               void* context);

    void ward_vz_install_macos(const char* restore_image_path,
                               const WardVzMacOSVirtualMachineConfiguration* configuration,
                               WardVzMacOSInstallationProgress progress,
                               WardVzInstallMacOSCompletion completion,
                               void* context);

    void ward_vz_create_macos_virtual_machine(const WardVzMacOSVirtualMachineConfiguration* configuration,
                                              const char* machine_state_path,
                                              const char* saving_machine_state_path,
                                              const char* restoring_machine_state_path,
                                              WardVzMacOSVirtualMachineEvent event,
                                              void* event_context,
                                              WardVzCreateMacOSVirtualMachineCompletion completion,
                                              void* completion_context);

    void ward_vz_destroy_macos_virtual_machine(WardVzMacOSVirtualMachineHandle* virtual_machine);

    void ward_vz_start_macos_virtual_machine(WardVzMacOSVirtualMachineHandle* virtual_machine);

    void ward_vz_pause_macos_virtual_machine(WardVzMacOSVirtualMachineHandle* virtual_machine);

    void ward_vz_resume_macos_virtual_machine(WardVzMacOSVirtualMachineHandle* virtual_machine);

    void ward_vz_request_stop_macos_virtual_machine(WardVzMacOSVirtualMachineHandle* virtual_machine);

    void ward_vz_force_stop_macos_virtual_machine(WardVzMacOSVirtualMachineHandle* virtual_machine);

    void ward_vz_suspend_macos_virtual_machine(WardVzMacOSVirtualMachineHandle* virtual_machine);

    void ward_vz_restore_macos_virtual_machine(WardVzMacOSVirtualMachineHandle* virtual_machine);

    void ward_vz_discard_macos_virtual_machine_saved_state(WardVzMacOSVirtualMachineHandle* virtual_machine);

    void ward_vz_create_macos_virtual_machine_display(WardVzMacOSVirtualMachineHandle* virtual_machine,
                                                      WardVzCreateMacOSVirtualMachineDisplayCompletion completion,
                                                      void* context);

    void ward_vz_destroy_macos_virtual_machine_display(WardVzMacOSVirtualMachineDisplayHandle* display);

#ifdef __cplusplus
}
#endif
