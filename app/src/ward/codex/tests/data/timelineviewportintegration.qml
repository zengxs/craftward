// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import "../../../../../qml/Craftward/Pages" as Pages

Item {
    width: 800
    height: 600

    Pages.CodexTimelineViewport {
        objectName: "integrationViewport"
        anchors.fill: parent
        timelineModel: presentationModel
        rowDelegate: rowComponent
        bottomContentInset: 0
        estimatedRowHeight: 80
        followLiveTail: false
    }

    Component {
        id: rowComponent

        Item {
            property int sourceRow: -1
            property int dataRevision: -1
            readonly property string entryId: {
                const currentRevision = dataRevision;
                return currentRevision >= 0 ? String(presentationModel.entryIdAt(sourceRow)) : "";
            }

            implicitHeight: 80
        }
    }
}
