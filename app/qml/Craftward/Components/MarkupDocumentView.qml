// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls

Control {
    id: root

    required property var documentModel
    property color textColor: palette.text
    property font codeFont: font
    readonly property var renderModel: documentModel ? documentModel["renderModel"] : null

    function prepareForLayout() {
        const prepare = documentModel ? documentModel["prepareForLayout"] : null;
        if (typeof prepare === "function")
            prepare.call(documentModel);
        segmentColumn.forceLayout();
    }

    padding: 0
    implicitWidth: 0
    implicitHeight: segmentColumn.implicitHeight
    background: null

    contentItem: Column {
        id: segmentColumn

        width: root.availableWidth
        spacing: 8

        Repeater {
            model: root.renderModel

            delegate: MarkupSegmentView {
                required property string segmentId

                width: segmentColumn.width
                textColor: root.textColor
                font: root.font
                codeFont: root.codeFont
            }
        }
    }
}
