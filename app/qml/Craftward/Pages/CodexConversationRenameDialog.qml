// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Components

ModalDialog {
    id: root

    required property string currentName
    required property bool renameAllowed

    signal renameRequested(string name)

    function begin() {
        renameState.reset(currentName);
        open();
    }

    anchors.centerIn: Overlay.overlay
    width: Math.min(440, Overlay.overlay.width - 48)
    onOpened: Qt.callLater(() => {
        nameField.forceActiveFocus();
        nameField.selectAll();
    })

    CodexConversationRenameState {
        id: renameState

        renameAllowed: root.renameAllowed
        onRenameRequested: name => root.renameRequested(name)
    }

    contentItem: ColumnLayout {
        spacing: 12

        Label {
            Layout.fillWidth: true
            text: qsTr("Rename conversation")
            font.pixelSize: 18
            font.weight: Font.DemiBold
        }

        Label {
            Layout.fillWidth: true
            text: qsTr("Conversation name")
        }

        TextField {
            id: nameField

            Layout.fillWidth: true
            text: renameState.draft
            Accessible.name: qsTr("Conversation name")
            onTextEdited: renameState.draft = text
            Keys.onReturnPressed: event => {
                event.accepted = renameState.submit();
            }
        }

        Item {
            Layout.preferredHeight: 4
        }

        RowLayout {
            Layout.fillWidth: true

            Item {
                Layout.fillWidth: true
            }

            Button {
                text: qsTr("Cancel")
                onClicked: root.reject()
            }

            PrimaryButton {
                text: qsTr("Rename")
                enabled: renameState.canSubmit
                onClicked: renameState.submit()
            }
        }
    }
}
