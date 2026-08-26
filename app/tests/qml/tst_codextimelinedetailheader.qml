// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite

    width: 920
    height: 100

    Pages.CodexTimelineDetailHeader {
        id: header

        width: suite.width
        durationMilliseconds: 113000
        detailCount: 19
        expanded: false
        timerIconSource: ""
    }

    TestCase {
        name: "CodexTimelineDetailHeader"
        when: windowShown

        function test_keepsTheDisclosurePartsTogether() {
            const badge = findChild(header, "codexTimelineElapsedBadge");
            const chevron = findChild(header, "codexTimelineDisclosureChevron");
            verify(badge !== null);
            verify(chevron !== null);
            compare(chevron.x - (badge.x + badge.width), 6);
            verify(chevron.x + chevron.width < header.width / 2);
        }
    }
}
