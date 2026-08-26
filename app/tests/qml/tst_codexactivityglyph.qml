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
            const expectations = [["reasoning", "qrc:///icons/fluent/lightbulb-20-regular.svg"], ["plan", "qrc:///icons/fluent/lightbulb-20-regular.svg"], ["readFiles", "qrc:///icons/fluent/book-open-20-regular.svg"], ["listFiles", "qrc:///icons/fluent/folder-20-regular.svg"], ["searchFiles", "qrc:///icons/fluent/folder-search-20-regular.svg"], ["runCommands", "qrc:///icons/fluent/window-console-20-regular.svg"], ["fileChange", "qrc:///icons/fluent/edit-20-regular.svg"], ["webSearch", "qrc:///icons/fluent/globe-search-20-regular.svg"], ["contextCompaction", "qrc:///icons/fluent/square-text-arrow-repeat-all-20-regular.svg"]];

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
