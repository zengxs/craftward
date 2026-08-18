// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    id: root

    property bool archived
    property bool hasSelection
    property bool loadingThreads
    property bool loadingConversation
    property bool startingThread
    property bool turnInFlight
    property bool changingThreadLifecycle

    readonly property bool busy: loadingThreads || loadingConversation || startingThread || turnInFlight || changingThreadLifecycle
    readonly property bool canSwitchScope: !busy
    readonly property bool canStartThread: !archived && !busy
    readonly property bool renameAllowed: hasSelection && !archived && !busy
    readonly property bool canArchive: hasSelection && !archived && !busy
    readonly property bool canRestore: hasSelection && archived && !busy
    readonly property bool composerVisible: hasSelection && !archived
}
