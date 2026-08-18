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

        Pages.CodexHistoryActionState {}
    }

    TestCase {
        name: "CodexHistoryActionState"

        function init() {
            suite.state = stateComponent.createObject(suite);
            verify(suite.state !== null);
        }

        function cleanup() {
            suite.state.destroy();
            suite.state = null;
        }

        function test_activeSelectionAllowsWritingActions() {
            suite.state.hasSelection = true;
            suite.state.forkReady = true;

            verify(suite.state.canSwitchScope);
            verify(suite.state.canStartThread);
            verify(suite.state.renameAllowed);
            verify(suite.state.canFork);
            verify(suite.state.canArchive);
            verify(!suite.state.canRestore);
            verify(suite.state.composerVisible);
        }

        function test_archivedSelectionIsStrictlyReadOnly() {
            suite.state.archived = true;
            suite.state.hasSelection = true;
            suite.state.forkReady = true;

            verify(suite.state.canSwitchScope);
            verify(!suite.state.canStartThread);
            verify(!suite.state.renameAllowed);
            verify(!suite.state.canFork);
            verify(!suite.state.canArchive);
            verify(suite.state.canRestore);
            verify(!suite.state.composerVisible);
        }

        function test_forkRequiresAnEligibleRuntimeAndWriterState() {
            suite.state.hasSelection = true;

            verify(!suite.state.canFork);
            suite.state.forkReady = true;
            verify(suite.state.canFork);
        }

        function test_busyHistoryBlocksScopeAndLifecycleChanges() {
            suite.state.hasSelection = true;
            suite.state.forkReady = true;
            suite.state.loadingThreads = true;

            verify(!suite.state.canSwitchScope);
            verify(!suite.state.canArchive);

            suite.state.loadingThreads = false;
            suite.state.changingThreadLifecycle = true;
            verify(!suite.state.canSwitchScope);
            verify(!suite.state.canArchive);

            suite.state.changingThreadLifecycle = false;
            suite.state.forkingThread = true;
            verify(suite.state.busy);
            verify(!suite.state.canFork);
        }
    }
}
