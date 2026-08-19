// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick

CodexCatalogSelector {
    id: root

    required property var catalogModel
    property string selectedModel
    property bool loading: false
    property string errorMessage
    readonly property bool catalogReady: !loading && errorMessage.length === 0 && count > 0
    readonly property bool selectedModelIsListed: selectedValueIsListed

    signal modelSelected(string model)

    selectedValue: selectedModel
    choicesReady: catalogReady
    model: catalogModel
    textRole: "displayName"
    valueRole: "model"
    displayText: {
        if (selectedModelIsListed)
            return currentText;
        if (selectedModel.length > 0)
            return selectedModel;
        if (loading)
            return qsTr("Loading models…");
        if (errorMessage.length > 0)
            return qsTr("Models unavailable");
        if (count > 0)
            return qsTr("Current model");
        return qsTr("No models available");
    }
    Accessible.name: qsTr("Conversation model")
    onValueSelected: value => root.modelSelected(value)

    toolTipText: {
        if (errorMessage.length > 0)
            return errorMessage;
        if (selectedModel.length > 0 && !selectedModelIsListed)
            return qsTr("This conversation uses a model that is not in the current catalog.");
        return qsTr("The selected model is applied when the next turn starts and remains active for later turns.");
    }
}
