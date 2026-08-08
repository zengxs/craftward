import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Page {
    id: root

    property url applicationIconSource
    property string buildNumber
    property string commitHash
    readonly property bool hasApplicationIcon: applicationIconSource.toString().length > 0
    readonly property string versionSummary: {
        const parts = [qsTr("Version %1").arg(Application.version)];
        if (root.buildNumber.length > 0)
            parts.push(qsTr("Build %1").arg(root.buildNumber));
        if (root.commitHash.length > 0)
            parts.push(qsTr("Commit %1").arg(root.commitHash));
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
                Layout.preferredWidth: visible ? 96 : 0
                Layout.preferredHeight: visible ? 96 : 0
                source: root.applicationIconSource
                visible: root.hasApplicationIcon
                asynchronous: true
                fillMode: Image.PreserveAspectFit
                mipmap: true
            }

            Label {
                Layout.fillWidth: true
                Layout.topMargin: applicationIcon.visible ? 14 : 0
                text: qsTr("Craftward")
                font.pixelSize: 22
                font.weight: Font.DemiBold
                horizontalAlignment: Text.AlignHCenter
            }

            Label {
                Layout.fillWidth: true
                Layout.topMargin: 4
                text: qsTr("From intent to artifact.")
                font.pixelSize: 13
                color: root.palette.placeholderText
                horizontalAlignment: Text.AlignHCenter
            }

            Label {
                Layout.fillWidth: true
                Layout.topMargin: 8
                text: root.versionSummary
                font.family: "Menlo"
                font.pixelSize: 11
                color: root.palette.placeholderText
                horizontalAlignment: Text.AlignHCenter
            }

            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: 20
                spacing: 8

                Button {
                    text: qsTr("License")
                    onClicked: root.viewApplicationLicenseRequested()
                }

                Button {
                    text: qsTr("Source Code")
                    onClicked: Qt.openUrlExternally("https://github.com/zengxs/craftward")
                }

                Button {
                    text: qsTr("Third-Party Licenses")
                    onClicked: root.viewThirdPartyLicensesRequested()
                }
            }

            Label {
                Layout.fillWidth: true
                Layout.topMargin: 24
                text: qsTr("Copyright © 2026 Xiangsong Zeng.")
                font.pixelSize: 12
                horizontalAlignment: Text.AlignHCenter
            }

            Label {
                Layout.fillWidth: true
                Layout.topMargin: 10
                text: qsTr("Licensed under the GNU General Public License,\nversion 3 or later.")
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
                text: qsTr("This program comes with absolutely no warranty.")
                font.pixelSize: 12
                color: root.palette.placeholderText
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
            }
        }

        ScrollBar.vertical: ScrollBar {
            policy: ScrollBar.AsNeeded
        }
    }
}
