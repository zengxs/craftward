// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtTest
import Craftward.Components
import Craftward.Design

Item {
    id: suite

    width: 420
    height: 240

    property var selector

    Component {
        id: selectorComponent

        MenuComboBox {
            x: 60
            y: 100
            width: 300
            model: [
                {
                    "label": "One"
                },
                {
                    "label": "Two"
                },
                {
                    "label": "Three"
                }
            ]
            textRole: "label"
            currentIndex: 1
            optionText: value => "[" + value + "]"
        }
    }

    SignalSpy {
        id: activatedSpy

        target: suite.selector
        signalName: "activated"
    }

    TestCase {
        name: "MenuComboBox"
        when: windowShown

        function init() {
            suite.selector = selectorComponent.createObject(suite);
            verify(suite.selector !== null);
            activatedSpy.clear();
        }

        function cleanup() {
            suite.selector.destroy();
            suite.selector = null;
        }

        function test_popupUsesWindowAndCompactRows() {
            compare(suite.selector.focusPolicy, Qt.TabFocus);
            compare(suite.selector.popup.popupType, Popup.Window);

            suite.selector.forceActiveFocus();
            suite.selector.popup.open();
            tryVerify(() => suite.selector.popup.opened);
            compare(suite.selector.popup.width, 280);
            compare(suite.selector.width - suite.selector.popup.width, 20);

            const popupList = suite.selector.popup.contentItem;
            tryCompare(popupList, "count", 3);
            compare(popupList.currentIndex, suite.selector.currentIndex);
            const selectedDelegate = popupList.itemAtIndex(1);
            verify(selectedDelegate !== null);
            compare(selectedDelegate.text, "[Two]");
            compare(selectedDelegate.height, 24);
            compare(selectedDelegate.background.color, Theme.menuSelectionBackground);
        }

        function test_delegateSelectionUsesTheComboBoxContract() {
            suite.selector.popup.open();
            tryVerify(() => suite.selector.popup.opened);

            const thirdDelegate = suite.selector.popup.contentItem.itemAtIndex(2);
            verify(thirdDelegate !== null);
            thirdDelegate.clicked();

            tryCompare(suite.selector, "currentIndex", 2);
            compare(activatedSpy.count, 1);
            compare(activatedSpy.signalArguments[0][0], 2);
            tryVerify(() => !suite.selector.popup.opened);
        }

        function test_popupExpandsForTheLongestOption() {
            suite.selector.width = 90;
            suite.selector.model = [
                {
                    "label": "A considerably longer option"
                }
            ];
            suite.selector.currentIndex = 0;

            suite.selector.popup.open();
            tryVerify(() => suite.selector.popup.opened);

            tryVerify(() => suite.selector.popup.width > suite.selector.width);
            const onlyDelegate = suite.selector.popup.contentItem.itemAtIndex(0);
            verify(onlyDelegate !== null);
            compare(onlyDelegate.text, "[A considerably longer option]");
        }
    }
}
