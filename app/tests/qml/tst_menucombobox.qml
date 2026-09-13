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
            tryCompare(popupList, "contentHeight", popupList.height);
            verify(!popupList.ScrollBar.vertical.visible);
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

        function test_longListKeepsTheSelectedOptionVisible() {
            suite.selector.maximumVisibleItems = 6;
            suite.selector.sectionRole = "group";
            suite.selector.model = Array.from({
                length: 40
            }, (_, index) => ({
                        label: "Font " + index,
                        group: index === 0 ? "Bundled fonts" : "System fonts"
                    }));
            suite.selector.currentIndex = 37;

            suite.selector.popup.open();
            tryVerify(() => suite.selector.popup.opened);
            const popupList = suite.selector.popup.contentItem;
            verify(popupList.contentHeight > popupList.height);
            verify(popupList.ScrollBar.vertical.visible);
            verify(suite.selector.popup.height < suite.height);
            tryVerify(() => {
                const selected = popupList.itemAtIndex(37);
                return selected !== null && selected.y >= popupList.contentY && selected.y + selected.height <= popupList.contentY + popupList.height;
            });

            const lastDelegate = popupList.itemAtIndex(39);
            verify(lastDelegate !== null);
            lastDelegate.clicked();
            tryCompare(suite.selector, "currentIndex", 39);
            compare(activatedSpy.count, 1);
            tryVerify(() => !suite.selector.popup.opened);
        }

        function test_keyboardSelectionSkipsSectionHeadings() {
            suite.selector.sectionRole = "group";
            suite.selector.valueRole = "label";
            // QuickTest sends keys to its test window, so keep this keyboard case in that window.
            suite.selector.popup.popupType = Popup.Item;
            suite.selector.model = [
                {
                    label: "Fira Code",
                    group: "Bundled fonts"
                },
                {
                    label: "Menlo",
                    group: "System fonts"
                },
                {
                    label: "Monaco",
                    group: "System fonts"
                }
            ];
            suite.selector.currentIndex = 0;
            suite.selector.forceActiveFocus();
            suite.selector.popup.open();
            tryVerify(() => suite.selector.popup.opened);

            compare(suite.selector.count, 3);
            keyClick(Qt.Key_Down);
            tryCompare(suite.selector, "highlightedIndex", 1);
            keyClick(Qt.Key_Return);
            tryCompare(suite.selector, "currentValue", "Menlo");
            compare(activatedSpy.count, 1);
            tryVerify(() => !suite.selector.popup.opened);
        }
    }
}
