// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import Craftward.Components

Item {
    id: suite

    width: 100
    height: 100

    AnimatedChevron {
        id: chevron

        expanded: false
    }

    TestCase {
        name: "AnimatedChevron"
        when: windowShown

        function test_rotatesBetweenCollapsedAndExpandedStates() {
            compare(chevron.rotation, 0);

            chevron.expanded = true;
            tryCompare(chevron, "rotation", 90, 300);

            chevron.expanded = false;
            tryCompare(chevron, "rotation", 0, 300);
        }
    }
}
