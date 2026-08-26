// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    property var timeline
    property string threadId
    property bool loading: false
    property bool hasRunningEvidence: false
    property bool turnRunning: false

    signal selectionChanged
    signal turnStarted
}
