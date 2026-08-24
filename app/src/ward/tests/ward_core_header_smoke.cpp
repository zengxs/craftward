// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include <ward_core.h>

#include <type_traits>

static_assert(std::is_same_v<WardCodexHistoryEventCallback, void (*)(void*, const WardBuffer*)>);
static_assert(std::is_same_v<decltype(&ward_core_codex_execution_target_create_host),
                             WardCodexExecutionTarget* (*)(const char*, WardError**)>);
static_assert(std::is_same_v<decltype(&ward_core_codex_history_observer_open),
                             WardCodexHistoryObserver* (*)(const WardRuntime*,
                                                           const WardCodexExecutionTarget*,
                                                           WardCodexHistoryEventCallback,
                                                           void*,
                                                           WardError**)>);
static_assert(std::is_same_v<decltype(&ward_core_codex_history_observer_set_polling_enabled_async),
                             bool (*)(WardCodexHistoryObserver*, bool, WardError**)>);
static_assert(std::is_same_v<WardRealmEventCallback, void (*)(void*, const WardRealmEvent*)>);
static_assert(std::is_same_v<decltype(&ward_core_realm_start_async), bool (*)(WardRealm*, WardError**)>);
static_assert(
  std::is_same_v<decltype(&ward_core_blake3_hash_file), bool (*)(const char*, WardBlake3Digest*, WardError**)>);

void
wardCoreHeaderCxxSmoke()
{
    const WardCodexTurnAttachment attachment{
        .kind = WardCodexTurnAttachmentKindMention,
        .name = "notes.txt",
        .path = "/workspace/notes.txt",
    };
    const WardRealmEvent event{};

    (void)attachment;
    (void)event;
}
