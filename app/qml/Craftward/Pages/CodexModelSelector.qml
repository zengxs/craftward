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
            return /*% "Loading models…" */ qsTrId("craftward.codex.model.loading");
        if (errorMessage.length > 0)
            return /*% "Models unavailable" */ qsTrId("craftward.codex.model.unavailable");
        if (count > 0)
            return /*% "Current model" */ qsTrId("craftward.codex.model.current");
        return /*% "No models available" */ qsTrId("craftward.codex.model.empty");
    }
    Accessible.name: /*% "Conversation model" */ qsTrId("craftward.codex.model.accessible_name")
    onValueSelected: value => root.modelSelected(value)

    toolTipText: {
        if (errorMessage.length > 0)
            return errorMessage;
        if (selectedModel.length > 0 && !selectedModelIsListed)
            return /*% "This conversation uses a model that is not in the current catalog." */ qsTrId("craftward.codex.model.not_in_catalog");
        return /*% "The selected model is applied when the next turn starts and remains active for later turns." */ qsTrId("craftward.codex.model.selection_description");
    }
}
