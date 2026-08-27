// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl as ControlsImpl

ToolButton {
    id: root

    property int composerAction: CodexComposerAction.SendAction
    property string statusText
    property int displayedAction: CodexComposerAction.SendAction
    property int pendingAction: composerAction
    property bool transitionReady: false
    readonly property url displayedIconSource: actionDescriptor.iconSource(displayedAction)
    readonly property string defaultActionText: actionDescriptor.label(composerAction)
    readonly property string toolTipText: statusText.length > 0 ? statusText : defaultActionText

    implicitWidth: 32
    implicitHeight: 32
    padding: 7
    display: AbstractButton.IconOnly
    hoverEnabled: true
    Accessible.name: toolTipText

    CodexComposerAction {
        id: actionDescriptor
    }

    Component.onCompleted: {
        displayedAction = composerAction;
        transitionReady = true;
    }

    onComposerActionChanged: {
        pendingAction = composerAction;
        if (!transitionReady) {
            displayedAction = composerAction;
            return;
        }
        actionTransition.restart();
    }

    contentItem: ControlsImpl.IconImage {
        id: actionGlyph

        source: root.displayedIconSource
        sourceSize.width: 20
        sourceSize.height: 20
        color: root.palette.base
        opacity: root.enabled ? 1 : 0.58
    }

    background: Rectangle {
        radius: width / 2
        color: {
            const foreground = root.palette.text;
            let opacity = root.enabled ? 1 : 0.24;
            if (root.enabled && root.down)
                opacity = 0.78;
            else if (root.enabled && root.hovered)
                opacity = 0.88;
            return Qt.rgba(foreground.r, foreground.g, foreground.b, opacity);
        }
        border.width: root.visualFocus ? 2 : 0
        border.color: root.palette.highlight

        Behavior on color {
            ColorAnimation {
                duration: 70
            }
        }
    }

    SequentialAnimation {
        id: actionTransition

        ParallelAnimation {
            NumberAnimation {
                target: actionGlyph
                property: "opacity"
                to: 0
                duration: 45
                easing.type: Easing.InQuad
            }
            NumberAnimation {
                target: actionGlyph
                property: "scale"
                to: 0.84
                duration: 45
                easing.type: Easing.InQuad
            }
        }
        ScriptAction {
            script: root.displayedAction = root.pendingAction
        }
        ParallelAnimation {
            NumberAnimation {
                target: actionGlyph
                property: "opacity"
                to: root.enabled ? 1 : 0.58
                duration: 55
                easing.type: Easing.OutQuad
            }
            NumberAnimation {
                target: actionGlyph
                property: "scale"
                to: 1
                duration: 55
                easing.type: Easing.OutQuad
            }
        }
    }

    ToolTip {
        id: helpTag

        popupType: Popup.Window
        parent: root
        x: Math.round((root.width - width) / 2)
        y: -height - 5
        visible: root.hovered && !root.down && text.length > 0
        text: root.toolTipText
        delay: 500
        timeout: 5000
        leftPadding: 7
        rightPadding: 7
        topPadding: 4
        bottomPadding: 4
        font.pixelSize: 11

        contentItem: Text {
            text: helpTag.text
            font: helpTag.font
            color: helpTag.palette.toolTipText
        }

        background: Rectangle {
            radius: 5
            color: helpTag.palette.toolTipBase
        }
    }
}
