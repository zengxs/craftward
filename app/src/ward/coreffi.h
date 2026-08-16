// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

// Craftward is currently the only consumer of Ward Core's private C ABI. This
// consumer-side shim keeps that integration seam out of Core's public interface.

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

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

    typedef struct WardBuffer WardBuffer;
    typedef struct WardError WardError;
    typedef struct WardRuntime WardRuntime;

    // Every asynchronous handle created from a runtime must be destroyed before
    // the runtime itself. Runtime and handle destruction must occur outside a
    // Ward callback and outside the runtime's Tokio worker threads.
    WardRuntime* ward_core_runtime_create(WardError** error);
    void ward_core_runtime_destroy(WardRuntime* runtime);

    typedef struct WardCodexHistoryObserver WardCodexHistoryObserver;
    typedef void (*WardCodexHistoryEventCallback)(void* context, const WardBuffer* event);

    typedef enum WardCodexTurnMode
    {
        WardCodexTurnModeDefault = 0,
        WardCodexTurnModePlan = 1,
    } WardCodexTurnMode;

    typedef enum WardCodexPermissionPreset
    {
        WardCodexPermissionPresetInherit = 0,
        WardCodexPermissionPresetRequestApproval = 1,
        WardCodexPermissionPresetReadOnly = 2,
    } WardCodexPermissionPreset;

    // The serialized event buffer is borrowed until the callback returns. The
    // callback context must remain valid until observer destruction completes.
    WardCodexHistoryObserver* ward_core_codex_history_observer_open(const WardRuntime* runtime,
                                                                    const char* executable,
                                                                    WardCodexHistoryEventCallback callback,
                                                                    void* callback_context,
                                                                    WardError** error);
    bool ward_core_codex_history_observer_watch(WardCodexHistoryObserver* observer,
                                                const char* thread_id,
                                                WardError** error);
    bool ward_core_codex_history_observer_start_thread(WardCodexHistoryObserver* observer,
                                                       const char* working_directory,
                                                       WardError** error);
    bool ward_core_codex_history_observer_refresh(WardCodexHistoryObserver* observer, WardError** error);
    bool ward_core_codex_history_observer_acquire_write(WardCodexHistoryObserver* observer,
                                                        const char* thread_id,
                                                        WardError** error);
    bool ward_core_codex_history_observer_release_write(WardCodexHistoryObserver* observer,
                                                        const char* thread_id,
                                                        WardError** error);
    bool ward_core_codex_history_observer_start_turn(WardCodexHistoryObserver* observer,
                                                     const char* thread_id,
                                                     const char* prompt,
                                                     WardCodexTurnMode turn_mode,
                                                     WardCodexPermissionPreset permission_preset,
                                                     WardError** error);
    bool ward_core_codex_history_observer_interrupt_turn(WardCodexHistoryObserver* observer,
                                                         const char* thread_id,
                                                         WardError** error);
    bool ward_core_codex_history_observer_resolve_interaction(WardCodexHistoryObserver* observer,
                                                              const uint8_t* response_data,
                                                              size_t response_size,
                                                              WardError** error);
    // Destruction waits for in-flight work and must not run inside the callback
    // or on a Tokio worker belonging to this observer's runtime.
    void ward_core_codex_history_observer_destroy(WardCodexHistoryObserver* observer);

    const uint8_t* ward_core_buffer_data(const WardBuffer* buffer);
    size_t ward_core_buffer_size(const WardBuffer* buffer);

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
