import QtQuick
import QtQuick.Controls
import Craftward.Components
import Craftward.Pages

ApplicationWindow {
    id: window

    property url applicationIconSource
    property string buildNumber
    property string commitHash
    property var settingsWindow: null

    function presentSettings(pageIndex) {
        if (!window.settingsWindow) {
            window.settingsWindow = settingsWindowComponent.createObject(window, {
                "applicationIconSource": window.applicationIconSource,
                "buildNumber": window.buildNumber,
                "commitHash": window.commitHash
            });
        }

        window.settingsWindow.present(pageIndex);
    }

    width: 960
    height: 640
    minimumWidth: 640
    minimumHeight: 480
    flags: Qt.Window | Qt.ExpandedClientAreaHint | Qt.NoTitleBarBackgroundHint
    visible: true
    title: qsTr("Craftward")

    menuBar: MenuBar {
        Menu {
            title: qsTr("File")

            Action {
                text: qsTr("Settings…")
                shortcut: StandardKey.Preferences
                onTriggered: window.presentSettings(0)
            }

            MenuSeparator {}

            Action {
                text: qsTr("Quit Craftward")
                shortcut: StandardKey.Quit
                onTriggered: Qt.quit()
            }
        }

        Menu {
            title: qsTr("Help")

            Action {
                text: qsTr("About Craftward")
                onTriggered: window.presentSettings(1)
            }
        }
    }

    background: Rectangle {
        color: window.palette.window

        WindowMoveHandler {
            targetWindow: window
        }
    }

    StackView {
        id: stackView

        anchors.fill: parent
        initialItem: ScaffoldPage {}
    }

    Component {
        id: settingsWindowComponent

        SettingsWindow {}
    }
}
