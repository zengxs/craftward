// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls.impl as ControlsImpl

Item {
    id: root

    required property string presentationKind
    required property color glyphColor
    readonly property string iconSource: root.sourceForPresentationKind(root.presentationKind)
    readonly property bool hasDedicatedIcon: root.iconSource.length > 0

    function sourceForPresentationKind(kind) {
        switch (kind) {
        case "reasoning":
        case "plan":
            return "qrc:///icons/fluent/lightbulb-20-regular.svg";
        case "readFiles":
            return "qrc:///icons/fluent/book-open-20-regular.svg";
        case "listFiles":
            return "qrc:///icons/fluent/folder-20-regular.svg";
        case "searchFiles":
            return "qrc:///icons/fluent/folder-search-20-regular.svg";
        case "runCommands":
            return "qrc:///icons/fluent/window-console-20-regular.svg";
        case "fileChange":
            return "qrc:///icons/fluent/edit-20-regular.svg";
        case "webSearch":
            return "qrc:///icons/fluent/globe-search-20-regular.svg";
        case "contextCompaction":
            return "qrc:///icons/fluent/square-text-arrow-repeat-all-20-regular.svg";
        default:
            return "";
        }
    }

    implicitWidth: 16
    implicitHeight: 16

    ControlsImpl.IconImage {
        anchors.centerIn: parent
        width: 16
        height: 16
        source: root.iconSource
        sourceSize.width: 20
        sourceSize.height: 20
        color: root.glyphColor
        visible: root.hasDedicatedIcon
    }

    Rectangle {
        anchors.centerIn: parent
        width: 7
        height: 7
        radius: width / 2
        color: root.glyphColor
        visible: !root.hasDedicatedIcon
    }
}
