import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Localization

Page {
    id: root

    required property LocalizationController localizationController

    // Language endonyms remain readable independently of the current UI language.
    readonly property var languageOptions: [
        {
            //% "System"
            text: qsTrId("craftward.settings.language.system"),
            value: LocalizationController.SystemLanguage
        },
        {
            text: "English",
            value: LocalizationController.English
        },
        {
            text: "简体中文",
            value: LocalizationController.SimplifiedChinese
        }
    ]

    background: Rectangle {
        color: root.palette.window
    }

    ColumnLayout {
        anchors {
            top: parent.top
            left: parent.left
            right: parent.right
            topMargin: 28
            leftMargin: 32
            rightMargin: 32
        }
        spacing: 8

        Label {
            Layout.fillWidth: true
            text: /*% "General" */ qsTrId("craftward.settings.general.title")
            font.pixelSize: 20
            font.weight: Font.DemiBold
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 12
            spacing: 16

            Label {
                Layout.fillWidth: true
                text: /*% "Language" */ qsTrId("craftward.settings.language.label")
                font.pixelSize: 13
            }

            ComboBox {
                id: languageSelector

                Layout.preferredWidth: 190
                model: root.languageOptions
                textRole: "text"
                valueRole: "value"
                currentIndex: {
                    for (let index = 0; index < root.languageOptions.length; ++index) {
                        if (root.languageOptions[index].value === root.localizationController.languagePreference)
                            return index;
                    }
                    return 0;
                }
                Accessible.name: /*% "Language" */ qsTrId("craftward.settings.language.label")
                onActivated: root.localizationController.languagePreference = currentValue
            }
        }
    }
}
