// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Effects
import QtQuick.Templates as T
import Craftward.Design

ComboBox {
    id: control

    property var optionText: function (value) {
        return value;
    }

    focusPolicy: Qt.TabFocus

    QtObject {
        id: metrics

        readonly property real rowHeight: 24
        readonly property real popupPadding: 5
        readonly property real popupRightReveal: 24
        readonly property real optionTextInset: 22
        readonly property real optionTextRightInset: 12
        readonly property int alignedIndex: control.currentIndex >= 0 ? control.currentIndex : 0
        readonly property real rowsHeight: control.count * rowHeight
        readonly property real selectedRowCenter: popupPadding + alignedIndex * rowHeight + rowHeight / 2
        readonly property real basePopupWidth: Math.max(1, control.width - 20)
        readonly property real contentPopupWidth: widestOptionText + optionTextInset + optionTextRightInset + 2 * popupPadding
        readonly property real popupWidth: Math.max(basePopupWidth, contentPopupWidth)
        readonly property real popupHorizontalOffset: control.width - popupRightReveal - popupWidth
        property real widestOptionText: 0

        function updateWidestOptionText() {
            let widest = 0;
            for (let index = 0; index < optionMeasurements.count; ++index) {
                const measurement = optionMeasurements.itemAt(index);
                if (measurement)
                    widest = Math.max(widest, measurement.implicitWidth);
            }
            widestOptionText = widest;
        }
    }

    Timer {
        id: measurementTimer

        interval: 0
        onTriggered: metrics.updateWidestOptionText()
    }

    Item {
        visible: false

        Repeater {
            id: optionMeasurements

            model: control.count

            delegate: Text {
                required property int index

                text: control.optionText(control.textAt(index))
                font: control.font
                onImplicitWidthChanged: measurementTimer.restart()
            }

            onItemAdded: measurementTimer.restart()
            onItemRemoved: measurementTimer.restart()
        }
    }

    delegate: T.ItemDelegate {
        id: option

        required property int index
        readonly property bool visuallyHighlighted: hovered || (control.activeFocus && highlighted)

        width: optionList.width
        height: metrics.rowHeight
        hoverEnabled: true
        highlighted: control.highlightedIndex === index
        text: control.optionText(control.textAt(index))

        contentItem: Item {
            Text {
                anchors.left: parent.left
                anchors.leftMargin: 3
                anchors.verticalCenter: parent.verticalCenter
                width: 16
                text: option.index === control.currentIndex ? "✓" : ""
                color: option.visuallyHighlighted ? Theme.menuSelectionForeground : control.palette.text
                font: control.font
                horizontalAlignment: Text.AlignHCenter
            }

            Text {
                anchors.left: parent.left
                anchors.leftMargin: metrics.optionTextInset
                anchors.right: parent.right
                anchors.rightMargin: metrics.optionTextRightInset
                anchors.verticalCenter: parent.verticalCenter
                text: option.text
                color: option.visuallyHighlighted ? Theme.menuSelectionForeground : control.palette.text
                font: control.font
                elide: Text.ElideRight
                verticalAlignment: Text.AlignVCenter
            }
        }

        background: Rectangle {
            radius: 7
            color: option.visuallyHighlighted ? Theme.menuSelectionBackground : "transparent"
        }
    }

    popup: Popup {
        id: menu

        x: metrics.popupHorizontalOffset
        y: control.height / 2 - metrics.selectedRowCenter
        width: metrics.popupWidth
        height: metrics.rowsHeight + topPadding + bottomPadding
        topPadding: metrics.popupPadding
        rightPadding: metrics.popupPadding
        bottomPadding: metrics.popupPadding
        leftPadding: metrics.popupPadding
        margins: -1
        leftInset: -28
        topInset: -24
        rightInset: -28
        bottomInset: -32
        modal: false
        dim: false
        popupType: Popup.Window
        onAboutToShow: metrics.updateWidestOptionText()

        contentItem: ListView {
            id: optionList

            model: control.delegateModel
            currentIndex: control.highlightedIndex >= 0 ? control.highlightedIndex : control.currentIndex
            clip: true
            boundsBehavior: Flickable.StopAtBounds
        }

        background: Item {
            id: popupBackground

            implicitWidth: 120 - menu.leftInset - menu.rightInset
            implicitHeight: 48 - menu.topInset - menu.bottomInset
            readonly property real panelX: -menu.leftInset
            readonly property real panelY: -menu.topInset
            readonly property real panelWidth: width + menu.leftInset + menu.rightInset
            readonly property real panelHeight: height + menu.topInset + menu.bottomInset

            MultiEffect {
                x: popupBackground.panelX
                y: popupBackground.panelY
                width: source.width
                height: source.height
                source: Rectangle {
                    width: popupBackground.panelWidth
                    height: popupBackground.panelHeight
                    radius: 14
                    visible: false
                    gradient: Gradient {
                        GradientStop {
                            position: 0
                            color: Application.styleHints.colorScheme === Qt.Dark ? "#2c2c30" : "#ffffff"
                        }
                        GradientStop {
                            position: 1
                            color: Application.styleHints.colorScheme === Qt.Dark ? "#202024" : "#f4f4f5"
                        }
                    }
                }
                shadowEnabled: true
                shadowBlur: 0.7
                shadowScale: 1
                shadowOpacity: Application.styleHints.colorScheme === Qt.Dark ? 0.3 : 0.16
                shadowColor: "black"
                shadowHorizontalOffset: 0
                shadowVerticalOffset: 6
            }

            Rectangle {
                x: popupBackground.panelX
                y: popupBackground.panelY
                width: popupBackground.panelWidth
                height: popupBackground.panelHeight
                radius: 14
                color: "transparent"
                border.width: 0.5
                border.color: Application.styleHints.colorScheme === Qt.Dark ? Qt.rgba(1, 1, 1, 0.42) : Qt.rgba(0, 0, 0, 0.25)

                Rectangle {
                    anchors.fill: parent
                    anchors.margins: 1
                    radius: 13
                    color: "transparent"
                    border.width: 0.5
                    border.color: Application.styleHints.colorScheme === Qt.Dark ? Qt.rgba(1, 1, 1, 0.16) : Qt.rgba(1, 1, 1, 0.58)
                }
            }
        }
    }
}
