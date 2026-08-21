// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite

    width: 320
    height: 120

    property var selector

    Component {
        id: selectorComponent

        Pages.CodexReasoningEffortSelector {
            efforts: [
                {
                    "reasoningEffort": "low",
                    "description": "Faster responses"
                },
                {
                    "reasoningEffort": "xhigh",
                    "description": "Deepest reasoning"
                }
            ]
        }
    }

    SignalSpy {
        id: selectedSpy
        target: suite.selector
        signalName: "effortSelected"
    }

    TestCase {
        name: "CodexReasoningEffortSelector"
        when: windowShown

        function init() {
            suite.selector = selectorComponent.createObject(suite);
            verify(suite.selector !== null);
            selectedSpy.clear();
        }

        function cleanup() {
            suite.selector.destroy();
            suite.selector = null;
        }

        function test_mapsSelectionAndEmitsCanonicalEffort() {
            suite.selector.selectedEffort = "xhigh";
            tryCompare(suite.selector, "currentIndex", 1);
            compare(suite.selector.displayText, "Extra high");
            verify(suite.selector.enabled);

            suite.selector.currentIndex = 0;
            suite.selector.activated(0);
            compare(selectedSpy.count, 1);
            compare(selectedSpy.signalArguments[0][0], "low");
        }

        function test_popupDelegatesDisplayFriendlyEffortNames() {
            suite.selector.popup.open();
            tryVerify(function () {
                return suite.selector.popup.opened;
            });

            const popupList = suite.selector.popup.contentItem;
            tryCompare(popupList, "count", 2);
            tryVerify(function () {
                return popupList.itemAtIndex(0) !== null && popupList.itemAtIndex(1) !== null;
            });
            const lowDelegate = popupList.itemAtIndex(0);
            const extraHighDelegate = popupList.itemAtIndex(1);
            verify(lowDelegate !== null);
            verify(extraHighDelegate !== null);
            compare(lowDelegate.text, "Low");
            compare(extraHighDelegate.text, "Extra high");
        }

        function test_preservesAnUnlistedEffortAndDisablesAnEmptyCatalog() {
            suite.selector.selectedEffort = "future";
            tryCompare(suite.selector, "currentIndex", -1);
            compare(suite.selector.displayText, "Future");
            verify(!suite.selector.selectedEffortIsListed);

            suite.selector.efforts = [];
            tryCompare(suite.selector, "count", 0);
            verify(!suite.selector.enabled);
            compare(suite.selector.displayText, "Future");
        }
    }
}
