import QtQuick
import QtQuick.Controls

Control {
    id: root

    property alias text: documentTextEdit.text
    property string errorMessage

    implicitWidth: 320
    implicitHeight: 240
    padding: 1

    background: Rectangle {
        radius: 5
        color: root.palette.base
        border.width: 1
        border.color: Qt.rgba(root.palette.windowText.r, root.palette.windowText.g, root.palette.windowText.b, 0.16)
    }

    contentItem: Item {
        Label {
            anchors.centerIn: parent
            width: Math.max(0, Math.min(parent.width - 48, 440))
            visible: root.errorMessage.length > 0
            text: root.errorMessage
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
        }

        Flickable {
            id: documentFlickable

            anchors.fill: parent
            visible: root.errorMessage.length === 0
            contentWidth: width
            contentHeight: Math.max(height, documentTextEdit.implicitHeight)
            flickableDirection: Flickable.VerticalFlick
            boundsBehavior: Flickable.DragAndOvershootBounds
            clip: true

            ScrollBar.horizontal: ScrollBar {
                policy: ScrollBar.AlwaysOff
            }

            ScrollBar.vertical: ScrollBar {
                id: verticalScrollBar

                implicitWidth: 10
                padding: 2
                policy: ScrollBar.AsNeeded

                background: Item {}

                contentItem: Rectangle {
                    id: verticalScrollThumb

                    implicitWidth: 6
                    implicitHeight: 6
                    radius: width / 2
                    color: root.palette.mid
                    opacity: 0

                    states: State {
                        name: "active"
                        when: verticalScrollBar.active || verticalScrollBar.hovered || verticalScrollBar.pressed

                        PropertyChanges {
                            verticalScrollThumb.opacity: 0.8
                        }
                    }

                    transitions: [
                        Transition {
                            to: "active"

                            NumberAnimation {
                                property: "opacity"
                                duration: 80
                            }
                        },
                        Transition {
                            from: "active"

                            SequentialAnimation {
                                PauseAnimation {
                                    duration: 450
                                }

                                NumberAnimation {
                                    property: "opacity"
                                    duration: 200
                                    to: 0
                                }
                            }
                        }
                    ]
                }
            }

            TextEdit {
                id: documentTextEdit

                width: documentFlickable.width
                height: documentFlickable.contentHeight
                leftPadding: 12
                rightPadding: 20
                topPadding: 10
                bottomPadding: 10
                font.family: "Menlo"
                color: root.palette.text
                selectionColor: root.palette.highlight
                selectedTextColor: root.palette.highlightedText
                readOnly: true
                selectByMouse: true
                wrapMode: TextEdit.WrapAtWordBoundaryOrAnywhere
                textFormat: TextEdit.PlainText
                onTextChanged: {
                    cursorPosition = 0;
                    documentFlickable.cancelFlick();
                    documentFlickable.contentY = 0;
                }
            }
        }
    }
}
