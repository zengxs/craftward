// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    property bool fromUser: false
    property bool finalAnswer: false
    property bool latestTurn: false
    property bool hasRunningEvidence: false
    property bool hovered: false
    property bool copyFeedbackActive: false
    property bool turnForkable: false
    property bool showForkActions: false

    readonly property bool messageEligible: fromUser || finalAnswer
    readonly property bool available: messageEligible && !(latestTurn && hasRunningEvidence)
    readonly property bool revealed: available && (latestTurn || hovered || copyFeedbackActive)
    readonly property bool forkVisible: available && finalAnswer && turnForkable && showForkActions
}
