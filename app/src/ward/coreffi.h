// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

// Craftward is currently the only consumer of Ward Core's private C ABI. This
// consumer-side shim keeps that integration seam out of Core's public interface.

#pragma once

#include <stdbool.h>

#ifdef __cplusplus
extern "C"
{
#endif

    typedef struct WardCliResult
    {
        bool handled;
        int exit_code;
    } WardCliResult;

    WardCliResult ward_core_cli_try_run(int argc, char** argv);

    typedef enum WardRealmState
    {
        WardRealmStateStopped = 0,
        WardRealmStateRunning = 1,
        WardRealmStatePaused = 2,
        WardRealmStateError = 3,
        WardRealmStateStarting = 4,
        WardRealmStatePausing = 5,
        WardRealmStateResuming = 6,
        WardRealmStateStopping = 7,
        WardRealmStateSaving = 8,
        WardRealmStateRestoring = 9,
        WardRealmStateSuspended = 10,
    } WardRealmState;

    typedef struct WardRealmStatus
    {
        int state;
        bool can_start;
        bool can_pause;
        bool can_resume;
        bool can_request_stop;
        bool can_force_stop;
        bool can_suspend;
        bool can_restore;
        bool can_discard_saved_state;
    } WardRealmStatus;

    typedef struct WardRealm WardRealm;
    typedef struct WardError WardError;

    typedef void (*WardRealmEvent)(void* context, const WardRealmStatus* status, const char* error_message);

    WardRealm* ward_core_realm_open(const char* bundle_path,
                                    WardRealmEvent event,
                                    void* event_context,
                                    WardError** error);

    void ward_core_realm_destroy(WardRealm* realm);

    void ward_core_realm_start(WardRealm* realm);
    void ward_core_realm_pause(WardRealm* realm);
    void ward_core_realm_resume(WardRealm* realm);
    void ward_core_realm_request_stop(WardRealm* realm);
    void ward_core_realm_force_stop(WardRealm* realm);
    void ward_core_realm_suspend(WardRealm* realm);
    void ward_core_realm_restore(WardRealm* realm);
    void ward_core_realm_discard_saved_state(WardRealm* realm);

    void* ward_core_realm_attach_display(WardRealm* realm, WardError** error);
    void ward_core_realm_detach_display(WardRealm* realm);

    const char* ward_core_error_message(const WardError* error);
    void ward_core_error_destroy(WardError* error);

#ifdef __cplusplus
}
#endif
