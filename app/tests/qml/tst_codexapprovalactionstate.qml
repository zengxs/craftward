// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite

    width: 320
    height: 180

    property var state

    Component {
        id: stateComponent

        Pages.CodexApprovalActionState {
            advertisedDecisions: []
            defaultDecisions: [1, 2, 3, 4]
        }
    }

    TestCase {
        name: "CodexApprovalActionState"

        function init() {
            suite.state = stateComponent.createObject(suite);
            verify(suite.state !== null);
        }

        function cleanup() {
            suite.state.destroy();
            suite.state = null;
        }

        function test_emptyAdvertisementUsesDefaultApprovalActions() {
            verify(suite.state.offers(1));
            verify(suite.state.offers(2));
            verify(suite.state.offers(3));
            verify(suite.state.offers(4));
        }

        function test_explicitAdvertisementRemainsAuthoritative() {
            suite.state.advertisedDecisions = [1, 3];

            verify(suite.state.offers(1));
            verify(!suite.state.offers(2));
            verify(suite.state.offers(3));
            verify(!suite.state.offers(4));
        }
    }
}
