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

        Pages.CodexMessageActionState {}
    }

    TestCase {
        name: "CodexMessageActionState"

        function init() {
            suite.state = stateComponent.createObject(suite);
            verify(suite.state !== null);
        }

        function cleanup() {
            suite.state.destroy();
            suite.state = null;
        }

        function test_revealsOlderAnswersOnlyWhileHovered() {
            suite.state.finalAnswer = true;
            suite.state.turnForkable = true;
            suite.state.showForkActions = true;

            verify(suite.state.available);
            verify(!suite.state.revealed);
            verify(suite.state.forkVisible);

            suite.state.hovered = true;
            verify(suite.state.revealed);
        }

        function test_revealsLatestCompletedMessagesWithoutHover() {
            suite.state.fromUser = true;
            suite.state.latestTurn = true;

            verify(suite.state.available);
            verify(suite.state.revealed);
            verify(!suite.state.forkVisible);
        }

        function test_hidesActionsForMessagesInARunningTurn() {
            suite.state.finalAnswer = true;
            suite.state.latestTurn = true;
            suite.state.hasRunningEvidence = true;
            suite.state.hovered = true;
            suite.state.turnForkable = true;
            suite.state.showForkActions = true;

            verify(!suite.state.available);
            verify(!suite.state.revealed);
            verify(!suite.state.forkVisible);
        }

        function test_runningNewerTurnDoesNotHideOlderMessages() {
            suite.state.finalAnswer = true;
            suite.state.latestTurn = false;
            suite.state.hasRunningEvidence = true;
            suite.state.hovered = true;

            verify(suite.state.available);
            verify(suite.state.revealed);
        }

        function test_copyFeedbackKeepsAnOlderMessageRevealed() {
            suite.state.fromUser = true;
            suite.state.copyFeedbackActive = true;

            verify(suite.state.revealed);
        }

        function test_nonfinalAgentMessagesHaveNoActions() {
            suite.state.hovered = true;

            verify(!suite.state.messageEligible);
            verify(!suite.state.available);
            verify(!suite.state.revealed);
        }
    }
}
