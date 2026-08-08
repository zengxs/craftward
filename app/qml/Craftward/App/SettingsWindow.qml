pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Components
import Craftward.Features.Legal
import Craftward.Pages

ApplicationWindow {
    id: root

    property url applicationIconSource
    property string buildNumber
    property string commitHash
    property int currentPage: 0
    readonly property real titleBarInset: SafeArea.margins.top

    function present(pageIndex) {
        root.currentPage = pageIndex;
        root.show();
        root.raise();
        root.requestActivate();
        if (pageIndex === 1)
            Qt.callLater(aboutPage.scrollToTop);
    }

    function showGeneral() {
        root.present(0);
    }

    function showAbout() {
        root.present(1);
    }

    component NavigationButton: Item {
        id: navigationButton

        property string text
        property bool selected: false
        signal clicked

        Layout.fillWidth: true
        implicitHeight: 32
        activeFocusOnTab: true
        Accessible.name: text
        Accessible.role: Accessible.Button

        Keys.onPressed: event => {
            if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                navigationButton.clicked();
                event.accepted = true;
            }
        }

        Rectangle {
            anchors.fill: parent
            radius: 6
            color: navigationButton.selected ? "#EEEEEE" : "transparent"
        }

        Label {
            anchors {
                fill: parent
                leftMargin: 10
                rightMargin: 10
            }
            text: navigationButton.text
            color: navigationButton.selected ? "#007AFF" : root.palette.text
            font.pixelSize: 13
            font.weight: Font.Normal
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }

        TapHandler {
            onTapped: {
                navigationButton.forceActiveFocus();
                navigationButton.clicked();
            }
        }
    }

    width: 740
    height: 480
    minimumWidth: 680
    minimumHeight: 420
    visible: false
    flags: Qt.Window | Qt.ExpandedClientAreaHint | Qt.NoTitleBarBackgroundHint
    title: ""
    topPadding: 0
    leftPadding: 0
    rightPadding: 0
    bottomPadding: 0

    background: Rectangle {
        color: root.palette.window
    }

    SplitView {
        anchors.fill: parent
        orientation: Qt.Horizontal
        handle: Rectangle {
            implicitWidth: 1
            color: root.palette.windowText
            opacity: 0.14
        }

        Rectangle {
            SplitView.minimumWidth: 180
            SplitView.preferredWidth: 200
            SplitView.maximumWidth: 260
            color: "#F8F8F8"

            WindowMoveHandler {
                targetWindow: root
            }

            ColumnLayout {
                anchors {
                    fill: parent
                    topMargin: root.titleBarInset + 10
                    leftMargin: 10
                    rightMargin: 10
                    bottomMargin: Math.max(12, root.SafeArea.margins.bottom)
                }
                spacing: 4

                NavigationButton {
                    text: qsTr("General")
                    selected: root.currentPage === 0
                    onClicked: root.currentPage = 0
                }

                Item {
                    Layout.fillHeight: true
                }

                NavigationButton {
                    text: qsTr("About")
                    selected: root.currentPage === 1
                    onClicked: {
                        root.currentPage = 1;
                        aboutPage.scrollToTop();
                    }
                }
            }
        }

        StackLayout {
            SplitView.fillWidth: true
            SplitView.fillHeight: true
            currentIndex: root.currentPage

            SettingsGeneralPage {}

            AboutPage {
                id: aboutPage

                applicationIconSource: root.applicationIconSource
                buildNumber: root.buildNumber
                commitHash: root.commitHash
                onViewApplicationLicenseRequested: applicationLicenseDialog.open()
                onViewThirdPartyLicensesRequested: thirdPartyLicensesDialog.open()
            }
        }
    }

    LicenseTextDialog {
        id: applicationLicenseDialog

        documentTitle: qsTr("Craftward License")
        documentUri: "qrc:///legal/GPL-3.0.txt"
    }

    ThirdPartyLicensesDialog {
        id: thirdPartyLicensesDialog
    }
}
