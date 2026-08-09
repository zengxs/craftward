// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C"
{
#endif

    bool ward_vz_is_supported(void);

    typedef struct WardVzMacOSBundleInfo
    {
        const char* build_version;
        uint64_t os_version_major;
        uint64_t os_version_minor;
        uint64_t os_version_patch;
        uint64_t minimum_cpu_count;
        uint64_t minimum_memory_size;
        uint64_t disk_size;
    } WardVzMacOSBundleInfo;

    typedef struct WardVzError
    {
        const char* domain;
        int64_t code;
        const char* message;
    } WardVzError;

    typedef void (*WardVzPrepareMacOSBundleCompletion)(void* context,
                                                       const WardVzMacOSBundleInfo* bundle_info,
                                                       const WardVzError* error);

    void ward_vz_prepare_macos_bundle(const char* restore_image_path,
                                      const char* destination_path,
                                      uint64_t disk_size,
                                      WardVzPrepareMacOSBundleCompletion completion,
                                      void* context);

#ifdef __cplusplus
}
#endif
