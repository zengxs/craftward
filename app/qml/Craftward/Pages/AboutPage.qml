import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Components

Page {
    id: root

    property url applicationIconSource
    property string buildNumber
    property string commitHash
    readonly property bool hasApplicationIcon: applicationIconSource.toString().length > 0
    readonly property string versionSummary: {
        const parts = [/*% "Version %1" */ qsTrId("craftward.about.version").arg(Application.version)];
        if (root.buildNumber.length > 0) {
            //% "Build %1"
            parts.push(qsTrId("craftward.about.build").arg(root.buildNumber));
        }
        if (root.commitHash.length > 0) {
            //% "Commit %1"
            parts.push(qsTrId("craftward.about.commit").arg(root.commitHash));
        }
        return parts.join(" · ");
    }

    signal viewApplicationLicenseRequested
    signal viewThirdPartyLicensesRequested

    function scrollToTop() {
        flickable.contentY = 0;
    }

    background: Rectangle {
        color: root.palette.window
    }

    Flickable {
        id: flickable

        anchors.fill: parent
        contentWidth: width
        contentHeight: Math.max(height, contentColumn.implicitHeight + 64)
        clip: true
        boundsBehavior: Flickable.StopAtBounds

        ColumnLayout {
            id: contentColumn

            x: Math.max(24, (flickable.width - width) / 2)
            y: Math.max(32, (flickable.height - implicitHeight) / 2)
            width: Math.min(460, flickable.width - 48)
            spacing: 0

            Image {
                id: applicationIcon

                Layout.alignment: Qt.AlignHCenter
                Layout.preferredWidth: visible ? 128 : 0
                Layout.preferredHeight: visible ? 128 : 0
                source: root.applicationIconSource
                visible: root.hasApplicationIcon
                asynchronous: true
                fillMode: Image.PreserveAspectFit
                mipmap: true
            }

            Label {
                Layout.fillWidth: true
                Layout.topMargin: applicationIcon.visible ? 14 : 0
                text: /*% "Craftward" */ qsTrId("craftward.app.name")
                font.pixelSize: 22
                font.weight: Font.DemiBold
                horizontalAlignment: Text.AlignHCenter
            }

            Label {
                Layout.fillWidth: true
                Layout.topMargin: 4
                text: /*% "From intent to artifact." */ qsTrId("craftward.about.tagline")
                font.pixelSize: 13
                color: root.palette.placeholderText
                horizontalAlignment: Text.AlignHCenter
            }

            Label {
                Layout.fillWidth: true
                Layout.topMargin: 8
                text: root.versionSummary
                font.family: Typography.monoFamily
                font.pixelSize: 11
                color: root.palette.placeholderText
                horizontalAlignment: Text.AlignHCenter
            }

            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: 20
                spacing: 8

                Button {
                    text: /*% "GitHub" */ qsTrId("craftward.about.github.action")
                    onClicked: Qt.openUrlExternally("https://github.com/zengxs/craftward")
                }

                Button {
                    text: /*% "License" */ qsTrId("craftward.about.license.action")
                    onClicked: root.viewApplicationLicenseRequested()
                }

                Button {
                    text: /*% "Third-Party Licenses" */ qsTrId("craftward.about.third_party_licenses.action")
                    onClicked: root.viewThirdPartyLicensesRequested()
                }
            }

            Label {
                Layout.fillWidth: true
                Layout.topMargin: 24
                text: /*% "Copyright © 2026 Xiangsong Zeng." */ qsTrId("craftward.about.copyright")
                font.pixelSize: 12
                horizontalAlignment: Text.AlignHCenter
            }

            Label {
                Layout.fillWidth: true
                Layout.topMargin: 10
                text: /*% "Licensed under the GNU General Public License,\nversion 3 or later." */ qsTrId("craftward.about.license_summary")
                font.pixelSize: 12
                color: root.palette.placeholderText
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
                lineHeight: 1.25
                lineHeightMode: Text.ProportionalHeight
            }

            Label {
                Layout.fillWidth: true
                Layout.topMargin: 8
                text: /*% "This program comes with absolutely no warranty." */ qsTrId("craftward.about.no_warranty")
                font.pixelSize: 12
                color: root.palette.placeholderText
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
            }
        }

        ScrollBar.vertical: OverlayScrollBar {}
    }
}
