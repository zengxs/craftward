pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Components
import Craftward.Design

ModalDialog {
    id: root

    property var components: []
    property var currentComponent: ({})
    property int currentIndex: -1
    property string documentText
    property string errorMessage
    property bool catalogLoaded: false

    function loadCatalog() {
        root.errorMessage = "";
        const result = ResourceTextReader.read("qrc:///legal/components.json");
        if (result.errorMessage.length > 0) {
            root.errorMessage = result.errorMessage;
            return;
        }

        let catalog;
        try {
            catalog = JSON.parse(result.text);
        } catch (error) {
            root.errorMessage = /*% "The legal document catalog is invalid." */ qsTrId("craftward.legal.error.catalog_invalid");
            return;
        }

        if (!catalog.components || catalog.components.length === 0) {
            root.errorMessage = /*% "No third-party software licenses are available." */ qsTrId("craftward.legal.third_party.empty");
            return;
        }

        root.catalogLoaded = true;
        root.components = catalog.components;
        root.selectComponent(0);
    }

    function selectComponent(index) {
        if (index < 0 || index >= root.components.length)
            return;

        const component = root.components[index];
        root.currentIndex = index;
        root.currentComponent = component;
        root.documentText = "";
        root.errorMessage = "";

        const result = ResourceTextReader.read(component.legalText);
        if (result.errorMessage.length > 0) {
            root.errorMessage = result.errorMessage;
            return;
        }

        root.documentText = result.text;
    }

    anchors.centerIn: Overlay.overlay
    width: Math.min(800, Overlay.overlay.width - 48)
    height: Math.min(560, Overlay.overlay.height - 48)
    visible: false
    title: /*% "Third-Party Component Licenses" */ qsTrId("craftward.legal.third_party.title")
    onOpened: if (!root.catalogLoaded)
        root.loadCatalog()

    contentItem: ColumnLayout {
        spacing: 12

        Label {
            Layout.fillWidth: true
            text: root.title
            font.pixelSize: 18
            font.weight: Font.DemiBold
            wrapMode: Text.WordWrap
        }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            Label {
                anchors.centerIn: parent
                width: Math.min(parent.width - 48, 440)
                visible: root.errorMessage.length > 0 && root.currentIndex < 0
                text: root.errorMessage
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
            }

            SplitView {
                anchors.fill: parent
                visible: root.currentIndex >= 0
                orientation: Qt.Horizontal

                ListView {
                    id: componentList

                    SplitView.minimumWidth: 150
                    SplitView.preferredWidth: 190
                    SplitView.maximumWidth: 260
                    topMargin: 4
                    bottomMargin: 4
                    clip: true
                    model: root.components

                    delegate: ItemDelegate {
                        id: componentDelegate

                        required property int index
                        required property var modelData

                        x: 4
                        width: componentList.width - 8
                        implicitHeight: 32
                        leftPadding: 10
                        rightPadding: 10
                        topPadding: 0
                        bottomPadding: 0
                        text: modelData.component
                        highlighted: index === root.currentIndex
                        hoverEnabled: true
                        onClicked: root.selectComponent(index)

                        contentItem: Label {
                            text: componentDelegate.text
                            color: componentDelegate.highlighted ? Theme.navigationSelectionForeground : root.palette.text
                            font.pixelSize: 13
                            font.weight: Font.Normal
                            verticalAlignment: Text.AlignVCenter
                            elide: Text.ElideRight
                        }

                        background: Rectangle {
                            radius: 5
                            color: {
                                if (componentDelegate.down)
                                    return Theme.navigationPressedBackground;
                                if (componentDelegate.highlighted)
                                    return Theme.navigationSelectionBackground;
                                return "transparent";
                            }
                            border.width: componentDelegate.visualFocus ? 1 : 0
                            border.color: Theme.accent
                        }
                    }

                    ScrollBar.vertical: OverlayScrollBar {}
                }

                ColumnLayout {
                    SplitView.fillWidth: true
                    SplitView.fillHeight: true
                    spacing: 10

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 6

                        Label {
                            text: root.currentComponent.component || ""
                            font.pixelSize: 20
                            font.weight: Font.DemiBold
                            wrapMode: Text.WordWrap
                        }

                        IconButton {
                            visible: Boolean(root.currentComponent.website)
                            icon.source: "qrc:///icons/phosphor/arrow-square-out.svg"
                            toolTipText: /*% "Open Project Website" */ qsTrId("craftward.legal.third_party.open_website")
                            onClicked: Qt.openUrlExternally(root.currentComponent.website)
                        }

                        Item {
                            Layout.fillWidth: true
                        }

                        Label {
                            text: root.currentComponent.spdxIdentifier || ""
                            font.pixelSize: 12
                            color: root.palette.placeholderText
                        }
                    }

                    LegalTextView {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        text: root.documentText
                        errorMessage: root.errorMessage
                    }
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true

            Item {
                Layout.fillWidth: true
            }

            Button {
                text: /*% "Close" */ qsTrId("craftward.action.close")
                onClicked: root.close()
            }
        }
    }
}
