// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include <ward_core.h>

static void
handle_codex_event(void* context, const WardBuffer* event)
{
    (void)context;
    (void)event;
}

static void
handle_realm_event(void* context, const WardRealmEvent* event)
{
    (void)context;
    (void)event;
}

void
ward_core_header_c_smoke(void)
{
    WardCodexHistoryEventCallback codex_callback = handle_codex_event;
    WardRealmEventCallback realm_callback = handle_realm_event;
    WardCodexTurnAttachment attachment = {
        .kind = WardCodexTurnAttachmentKindMention,
        .name = "notes.txt",
        .path = "/workspace/notes.txt",
    };
    WardRealmEvent event = { 0 };

    (void)codex_callback;
    (void)realm_callback;
    (void)attachment;
    (void)event;
}
