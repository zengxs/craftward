// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite

    width: 100
    height: 100

    Pages.CodexActivityGlyph {
        id: glyph

        presentationKind: "activity"
        glyphColor: "gray"
    }

    TestCase {
        name: "CodexActivityGlyph"
        when: windowShown

        function test_mapsDedicatedActivityIcons() {
            const expectations = [["reasoning", "qrc:///icons/fluent/lightbulb-20-regular.svg"], ["plan", "qrc:///icons/fluent/lightbulb-20-regular.svg"], ["readFiles", "qrc:///icons/hugeicons/book-open-02.svg"], ["listFiles", "qrc:///icons/hugeicons/folder-02.svg"], ["searchFiles", "qrc:///icons/hugeicons/search-01.svg"], ["runCommands", "qrc:///icons/hugeicons/square-terminal.svg"], ["fileChange", "qrc:///icons/hugeicons/edit-04.svg"], ["webSearch", "qrc:///icons/hugeicons/global-search.svg"], ["contextCompaction", "qrc:///icons/hugeicons/fold-vertical.svg"]];

            for (const expectation of expectations) {
                compare(glyph.sourceForPresentationKind(expectation[0]), expectation[1]);
            }
        }

        function test_keepsTheStatusDotForUnmappedActivities() {
            glyph.presentationKind = "activity";
            compare(glyph.sourceForPresentationKind("activity"), "");
            verify(!glyph.hasDedicatedIcon);
        }
    }
}
