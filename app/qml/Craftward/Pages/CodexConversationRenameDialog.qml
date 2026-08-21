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
            text: /*% "Rename conversation" */ qsTrId("craftward.codex.rename.title")
            font.pixelSize: 18
            font.weight: Font.DemiBold
        }

        Label {
            Layout.fillWidth: true
            text: /*% "Conversation name" */ qsTrId("craftward.codex.rename.name.label")
        }

        TextField {
            id: nameField

            Layout.fillWidth: true
            text: renameState.draft
            Accessible.name: /*% "Conversation name" */ qsTrId("craftward.codex.rename.name.label")
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
                text: /*% "Cancel" */ qsTrId("craftward.action.cancel")
                onClicked: root.reject()
            }

            PrimaryButton {
                text: /*% "Rename" */ qsTrId("craftward.action.rename")
                enabled: renameState.canSubmit
                onClicked: renameState.submit()
            }
        }
    }
}
