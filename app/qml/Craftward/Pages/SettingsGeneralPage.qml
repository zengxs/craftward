import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Components
import Craftward.Localization
import Craftward.Terminal

Page {
    id: root

    required property LocalizationController localizationController
    property TerminalController terminalController: null

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

            MenuComboBox {
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

        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 12
            visible: root.terminalController !== null
            Label {
                Layout.fillWidth: true
                text: /*% "Terminal font" */ qsTrId("craftward.terminal.font")
                font.pixelSize: 13
            }
            MenuComboBox {
                Layout.preferredWidth: 190
                maximumVisibleItems: 12
                textRole: "family"
                valueRole: "family"
                sectionRole: "group"
                model: {
                    if (!root.terminalController)
                        return [];
                    const bundled = /*% "Bundled fonts" */ qsTrId("craftward.terminal.fonts.bundled");
                    const system = /*% "System fonts" */ qsTrId("craftward.terminal.fonts.system");
                    return root.terminalController.fontFamilies.map(family => ({
                                family: family,
                                group: root.terminalController.bundledFontFamilies.indexOf(family) !== -1 ? bundled : system
                            }));
                }
                currentIndex: root.terminalController ? model.findIndex(option => option.family === root.terminalController.fontFamily) : -1
                Accessible.name: /*% "Terminal font" */ qsTrId("craftward.terminal.font")
                onActivated: root.terminalController.fontFamily = currentValue
            }
            SpinBox {
                from: 8
                to: 32
                editable: true
                value: root.terminalController ? root.terminalController.fontSize : 10
                Accessible.name: /*% "Terminal font size" */ qsTrId("craftward.terminal.font_size")
                onValueModified: root.terminalController.fontSize = value
            }
        }
    }
}
