// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite
    width: 280
    height: 400

    ListModel {
        id: sourceThreads
    }
    Component {
        id: listFactory
        Pages.CodexThreadList {
            width: suite.width
            height: suite.height
            threads: sourceThreads
            // Keep model updates independent of asynchronous cache incubation.
            cacheBuffer: 0
        }
    }
    TestCase {
        name: "CodexThreadList"
        when: windowShown
        property var list

        function init() {
            failOnWarning(/TypeError|ReferenceError|Binding loop/);
            sourceThreads.clear();
            for (let i = 0; i < 30; ++i)
                sourceThreads.append({
                    threadId: "thread-" + i,
                    title: "Thread " + i,
                    preview: "Conversation preview",
                    workingDirectory: "/project/" + i
                });
            list = createTemporaryObject(listFactory, suite);
            verify(list !== null);
            compare(list.count, 30);
            verify(waitForRendering(list));
        }

        function test_searchFindsOffscreenMatchesAfterScrolling() {
            list.positionViewAtEnd();
            verify(waitForRendering(list));
            verify(list.contentY > 0);
            list.searchText = "Thread 1";
            verify(waitForRendering(list));
            tryVerify(() => list.indexAt(1, list.contentY + 1) >= 0, 1000, "Matching offscreen conversations must remain visible after searching");
            compare(list.count, 11);
            const first = list.itemAtIndex(0);
            verify(first !== null);
            compare(first.title, "Thread 1");
        }

        function test_searchMatchesDirectoryWithoutCaseSensitivity() {
            list.searchText = "/PROJECT/29";
            tryCompare(list, "count", 1);
            verify(waitForRendering(list));
            compare(list.itemAtIndex(0).threadId, "thread-29");
            list.searchText = "";
            tryCompare(list, "count", 30);
        }

        function test_sourceChangesUpdateFilteredResults() {
            list.searchText = "renamed";
            tryCompare(list, "count", 0);
            verify(waitForRendering(list));
            sourceThreads.setProperty(5, "title", "Renamed conversation");
            tryCompare(list, "count", 1);
            verify(waitForRendering(list));
            compare(list.itemAtIndex(0).threadId, "thread-5");
            sourceThreads.setProperty(5, "title", "Thread 5");
            tryCompare(list, "count", 0);
            verify(waitForRendering(list));
            sourceThreads.append({
                threadId: "new",
                title: "Renamed new conversation",
                preview: "",
                workingDirectory: "/project"
            });
            tryCompare(list, "count", 1);
            verify(waitForRendering(list));
            compare(list.itemAtIndex(0).threadId, "new");
            sourceThreads.setProperty(6, "workingDirectory", "/renamed-project");
            tryCompare(list, "count", 2);
            verify(waitForRendering(list));
            compare(list.itemAtIndex(0).threadId, "thread-6");
        }
    }
}
