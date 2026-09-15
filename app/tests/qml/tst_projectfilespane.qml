// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import Craftward.TestSupport
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite
    width: 240
    height: 180

    Component {
        id: fixtureFactory
        FileTreeFixture {}
    }
    Component {
        id: paneFactory
        Pages.ProjectFilesPane {
            width: suite.width
            height: suite.height
        }
    }
    TestCase {
        name: "ProjectFilesPane"
        when: windowShown
        property var fixture
        property var pane
        property var tree

        function init() {
            failOnWarning(/TypeError|ReferenceError|Binding loop/);
            fixture = createTemporaryObject(fixtureFactory, suite);
            verify(fixture.directory.length > 0);
            for (let i = 0; i < 60; ++i)
                verify(fixture.createFile("a" + String(i).padStart(3, "0") + ".txt").length > 0);
        }

        function cleanup() {
            if (pane)
                pane.directory = "";
            pane = null;
            tree = null;
        }

        function createPane(minimumRows = 61) {
            pane = createTemporaryObject(paneFactory, suite, {
                directory: fixture.directory
            });
            verify(pane !== null);
            tree = findChild(pane, "projectFileTree");
            verify(tree !== null);
            tryVerify(() => tree.rows >= minimumRows);
        }

        function targetIsVisible(path) {
            const row = tree.rowAtIndex(tree.model.indexForPath(path));
            if (row < 0)
                return false;
            const item = tree.itemAtCell(Qt.point(0, row));
            return item !== null && item.y >= tree.contentY - 0.5 && item.y + item.height <= tree.contentY + tree.height + 0.5;
        }

        function test_firstRevealLoadsAndPositionsDeepFile_data() {
            return [
                {
                    tag: "collapsed",
                    siblingCount: 0
                },
                {
                    tag: "many-siblings",
                    siblingCount: 200
                }
            ];
        }

        function test_firstRevealLoadsAndPositionsDeepFile(data) {
            const parent = "zzz/level-one/level-two/";
            for (let i = 0; i < data.siblingCount; ++i)
                verify(fixture.createFile(parent + "sibling-" + i + ".txt").length > 0);
            const target = fixture.createFile(parent + "target.txt");
            verify(target.length > 0);
            createPane();
            pane.selectedPath = target;
            pane.reveal(target);
            tryCompare(tree, "rows", 64 + data.siblingCount);
            tryVerify(() => targetIsVisible(target), 2000, "The first reveal must bring the deep file into view after loading its ancestors");
        }

        function test_revealIncludesHiddenProjectPaths_data() {
            return [
                {
                    tag: "hidden-file",
                    path: ".gitignore"
                },
                {
                    tag: "hidden-ancestor",
                    path: ".github/workflows/ci.yml"
                },
                {
                    tag: "nested-hidden-file",
                    path: "zzz/.env"
                }
            ];
        }

        function test_revealIncludesHiddenProjectPaths(data) {
            const target = fixture.createFile(data.path);
            verify(target.length > 0);
            createPane(60);
            pane.selectedPath = target;
            pane.reveal(target);
            tryVerify(() => targetIsVisible(target), 2000, "Revealing a project file must include hidden targets and ancestors");
        }

        function test_revealWaitsForThePaneToBecomeVisible() {
            const target = fixture.createFile("zzz/level-one/level-two/target.txt");
            createPane();
            pane.visible = false;
            pane.reveal(target);
            verify(waitForRendering(suite));
            verify(!targetIsVisible(target));
            pane.visible = true;
            tryVerify(() => targetIsVisible(target));
        }

        function test_directoryChangeCancelsPendingReveal() {
            const target = fixture.createFile("zzz/level-one/level-two/target.txt");
            createPane();
            pane.visible = false;
            pane.reveal(target);
            verify(waitForRendering(suite));
            pane.directory = "";
            pane.directory = fixture.directory;
            pane.visible = true;
            verify(waitForRendering(pane));
            compare(tree.rows, 61);
            compare(tree.contentY, 0);
        }

        function test_latestRevealReplacesPendingTarget() {
            const target = fixture.createFile("zzz/level-one/level-two/target.txt");
            createPane();
            pane.visible = false;
            pane.reveal(target);
            verify(waitForRendering(suite));
            const replacement = fixture.directory + "/a000.txt";
            pane.reveal(replacement);
            pane.visible = true;
            tryVerify(() => targetIsVisible(replacement));
            compare(tree.rows, 61);
            verify(!targetIsVisible(target));
        }
    }
}
