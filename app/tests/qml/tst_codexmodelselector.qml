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

    ListModel {
        id: catalog
    }

    Component {
        id: selectorComponent

        Pages.CodexModelSelector {
            catalogModel: catalog
        }
    }

    SignalSpy {
        id: selectedSpy
        target: suite.selector
        signalName: "modelSelected"
    }

    TestCase {
        name: "CodexModelSelector"
        when: windowShown

        function init() {
            catalog.clear();
            catalog.append({
                "model": "gpt-balanced",
                "displayName": "Balanced"
            });
            catalog.append({
                "model": "gpt-fast",
                "displayName": "Fast"
            });
            suite.selector = selectorComponent.createObject(suite);
            verify(suite.selector !== null);
            selectedSpy.clear();
        }

        function cleanup() {
            suite.selector.destroy();
            suite.selector = null;
        }

        function test_mapsTheConversationSelectionAndEmitsCanonicalModel() {
            suite.selector.selectedModel = "gpt-fast";
            tryCompare(suite.selector, "currentIndex", 1);
            compare(suite.selector.displayText, "Fast");
            verify(suite.selector.catalogReady);
            verify(suite.selector.enabled);

            suite.selector.activated(1);
            compare(selectedSpy.count, 1);
            compare(selectedSpy.signalArguments[0][0], "gpt-fast");
        }

        function test_popupDelegatesDisplayCatalogNames() {
            suite.selector.popup.open();
            tryVerify(function () {
                return suite.selector.popup.opened;
            });

            const popupList = suite.selector.popup.contentItem;
            tryCompare(popupList, "count", 2);
            tryVerify(function () {
                return popupList.itemAtIndex(0) !== null && popupList.itemAtIndex(1) !== null;
            });
            const balancedDelegate = popupList.itemAtIndex(0);
            const fastDelegate = popupList.itemAtIndex(1);
            verify(balancedDelegate !== null);
            verify(fastDelegate !== null);
            compare(balancedDelegate.text, "Balanced");
            compare(fastDelegate.text, "Fast");
        }

        function test_preservesAnUnlistedActiveModelAndDisablesUnavailableCatalogs() {
            compare(suite.selector.displayText, "Current model");
            suite.selector.selectedModel = "gpt-legacy";
            tryCompare(suite.selector, "currentIndex", -1);
            compare(suite.selector.displayText, "gpt-legacy");
            verify(!suite.selector.selectedModelIsListed);

            suite.selector.loading = true;
            verify(!suite.selector.catalogReady);
            verify(!suite.selector.enabled);

            suite.selector.loading = false;
            suite.selector.errorMessage = "Catalog unavailable";
            verify(!suite.selector.catalogReady);
            verify(!suite.selector.enabled);
            compare(suite.selector.displayText, "gpt-legacy");
        }
    }
}
