import QtQuick
import QtQuick.Controls

ToolButton {
    id: control

    property string toolTipText

    implicitWidth: 28
    implicitHeight: 28
    padding: 6
    display: AbstractButton.IconOnly
    hoverEnabled: true
    icon.width: 16
    icon.height: 16
    Accessible.name: toolTipText

    background: Rectangle {
        radius: 5
        color: {
            const foreground = control.palette.buttonText;
            let opacity = 0;
            if (control.down)
                opacity = 0.14;
            else if (control.checked)
                opacity = 0.1;
            else if (control.hovered)
                opacity = 0.07;
            return Qt.rgba(foreground.r, foreground.g, foreground.b, opacity);
        }
        border.width: control.visualFocus ? 1 : 0
        border.color: control.palette.highlight

        Behavior on color {
            ColorAnimation {
                duration: 70
            }
        }
    }

    ToolTip {
        id: helpTag

        popupType: Popup.Window
        parent: control
        x: Math.round((control.width - width) / 2)
        y: control.height + 5
        visible: control.enabled && control.hovered && !control.down && text.length > 0
        text: control.toolTipText
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
