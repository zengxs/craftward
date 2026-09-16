import QtQuick
import QtQuick.Templates as T

T.ScrollBar {
    id: control

    implicitWidth: orientation === Qt.Vertical ? 10 : 0
    implicitHeight: orientation === Qt.Horizontal ? 10 : 0
    padding: 2
    hoverEnabled: true
    minimumSize: Math.min(1, 20 / Math.max(1, orientation === Qt.Vertical ? availableHeight : availableWidth))
    policy: T.ScrollBar.AsNeeded
    visible: policy === T.ScrollBar.AlwaysOn || (policy === T.ScrollBar.AsNeeded && size < 1)

    background: Item {}

    contentItem: Rectangle {
        id: thumb

        implicitWidth: 6
        implicitHeight: 6
        radius: Math.min(width, height) / 2
        color: control.palette.mid
        opacity: 0

        states: State {
            name: "active"
            when: control.policy === T.ScrollBar.AlwaysOn || control.active || control.hovered || control.pressed

            PropertyChanges {
                thumb.opacity: 0.8
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
