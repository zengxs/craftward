// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite

    width: 200
    height: 100

    Pages.CodexElapsedBadge {
        id: badge

        durationMilliseconds: 0
        timerIconSource: ""
    }

    TestCase {
        name: "CodexElapsedBadge"
        when: windowShown

        function test_usesMinuteSecondClockBelowOneHour() {
            compare(badge.formatClockDuration(0), "00:00");
            compare(badge.formatClockDuration(9000), "00:09");
            compare(badge.formatClockDuration((29 * 60 + 9) * 1000), "29:09");
            compare(badge.formatClockDuration((59 * 60 + 59) * 1000), "59:59");
        }

        function test_usesCumulativeHoursAtAndAboveOneHour() {
            compare(badge.formatClockDuration(60 * 60 * 1000), "01:00:00");
            compare(badge.formatClockDuration((24 * 60 * 60 + 7) * 1000), "24:00:07");
            compare(badge.formatClockDuration((123 * 60 * 60 + 4 * 60 + 5) * 1000), "123:04:05");
        }

        function test_keepsTheBadgeCompact() {
            compare(badge.implicitHeight, 22);
            verify(badge.implicitWidth > 0);
        }

        function test_describesElapsedTimeWithoutAnIncorrectAggregateCount() {
            badge.durationMilliseconds = 1000;

            compare(badge.description, "Elapsed 1 s");
        }

        function test_centersTheTimerAndClockTextVertically() {
            const timerIcon = findChild(badge, "codexElapsedBadgeTimerIcon");
            const clockText = findChild(badge, "codexElapsedBadgeClockText");
            verify(timerIcon !== null);
            verify(clockText !== null);
            compare(timerIcon.y + timerIcon.height / 2, clockText.y + clockText.height / 2);
        }
    }
}
