// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls

Menu {
    id: root

    property bool fileActive
    property bool hasConversation
    property bool archived
    property bool renameAllowed
    property bool archiveAllowed
    property bool restoreAllowed

    signal openFileRequested
    signal openExternalRequested
    signal refreshRequested
    signal renameRequested
    signal archiveRequested
    signal restoreRequested

    popupType: Popup.Window

    readonly property list<Action> availableActions: {
        const actions = [openFileAction];
        if (fileActive)
            return actions.concat([openExternalAction, refreshAction]);
        if (!archived)
            actions.push(renameAction);
        if (hasConversation)
            actions.push(archived ? restoreAction : archiveAction);
        return actions;
    }

    readonly property Action openFileAction: Action {
        text: /*% "Open File…" */ qsTrId("craftward.file.open")
        onTriggered: root.openFileRequested()
    }
    readonly property Action openExternalAction: Action {
        text: /*% "Open in Default Application" */ qsTrId("craftward.file.open_external")
        onTriggered: root.openExternalRequested()
    }
    readonly property Action refreshAction: Action {
        text: /*% "Refresh" */ qsTrId("craftward.action.refresh")
        onTriggered: root.refreshRequested()
    }
    readonly property Action renameAction: Action {
        text: /*% "Rename…" */ qsTrId("craftward.action.rename_ellipsis")
        enabled: root.renameAllowed
        onTriggered: root.renameRequested()
    }
    readonly property Action archiveAction: Action {
        text: /*% "Archive…" */ qsTrId("craftward.action.archive_ellipsis")
        enabled: root.archiveAllowed
        onTriggered: root.archiveRequested()
    }
    readonly property Action restoreAction: Action {
        text: /*% "Restore" */ qsTrId("craftward.action.restore")
        enabled: root.restoreAllowed
        onTriggered: root.restoreRequested()
    }

    Instantiator {
        model: root.availableActions
        delegate: MenuItem {
            required property Action modelData
            action: modelData
        }
        onObjectAdded: (index, object) => root.insertItem(index, object)
        onObjectRemoved: (index, object) => root.removeItem(object)
    }
}
