// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtTest
import Craftward.Components
import Craftward.TestSupport

Item {
    id: suite
    width: 1000
    height: 240

    AnimationClock {
        id: animationClock
    }

    Component {
        id: layoutFactory
        SplitView {
            id: split
            width: suite.width
            height: suite.height
            property alias leftPane: left
            property alias rightPane: right
            property alias centerPane: center
            handle: Rectangle {
                implicitWidth: 1
                containmentMask: Item {
                    x: -4
                    width: 9
                    height: split.height
                }
            }
            CollapsibleSplitPane {
                id: left
                resizing: split.resizing
                onResized: width => expandedWidth = width
            }
            Item {
                id: center
                SplitView.fillWidth: true
                SplitView.minimumWidth: 360
            }
            CollapsibleSplitPane {
                id: right
                expandedWidth: 240
                minimumExpandedWidth: 200
                maximumExpandedWidth: 360
                resizing: split.resizing
                onResized: width => expandedWidth = width
            }
        }
    }

    TestCase {
        name: "CollapsibleSplitPane"
        when: windowShown
        property var layout

        function init() {
            failOnWarning(/TypeError|ReferenceError|Binding loop/);
            layout = createTemporaryObject(layoutFactory, suite);
            verify(layout !== null);
            tryCompare(layout.leftPane, "width", 260);
            tryCompare(layout.rightPane, "width", 240);
            verify(waitForRendering(layout));
            animationClock.enable();
        }

        function cleanup() {
            animationClock.disable();
        }

        function advanceAnimation(milliseconds) {
            // Process animation startup before advancing the controlled clock.
            wait(0);
            let advanced = false;
            Qt.callLater(() => {
                advanced = animationClock.advance(milliseconds);
                suite.Window.window.update();
            });
            verify(waitForRendering(layout));
            verify(advanced);
        }

        function test_bothSidesAnimateAndReleaseSpace() {
            for (const pane of [layout.leftPane, layout.rightPane]) {
                const expandedWidth = pane.width;
                const centerWidth = layout.centerPane.width;
                pane.expanded = false;
                advanceAnimation(50);
                verify(pane.visible);
                verify(pane.width > 0 && pane.width < expandedWidth);
                verify(layout.centerPane.width > centerWidth);
                advanceAnimation(160);
                compare(pane.visible, false);
                pane.expanded = true;
                advanceAnimation(160);
                compare(pane.width, expandedWidth);
            }
        }

        function test_draggedWidthsSurviveCollapse() {
            for (const pane of [layout.leftPane, layout.rightPane]) {
                const before = pane.width;
                const handleX = pane === layout.leftPane ? pane.width : pane.x - 1;
                const distance = pane === layout.leftPane ? 50 : -50;
                mouseDrag(layout, handleX, 100, distance, 0);
                tryVerify(() => pane.width > before + 20);
                const resizedWidth = pane.width;
                compare(pane.expandedWidth, resizedWidth);
                pane.expanded = false;
                advanceAnimation(160);
                compare(pane.visible, false);
                pane.expanded = true;
                advanceAnimation(160);
                compare(pane.width, resizedWidth);
            }
        }

        function test_toggleCanReverseAnUnfinishedTransition() {
            const pane = layout.leftPane;
            pane.expanded = false;
            advanceAnimation(50);
            verify(pane.width > 0 && pane.width < 260);
            pane.expanded = true;
            advanceAnimation(160);
            compare(pane.width, 260);
            verify(pane.visible);
            compare(pane.expandedWidth, 260);
        }
    }
}
