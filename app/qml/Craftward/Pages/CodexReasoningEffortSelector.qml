// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick

CodexCatalogSelector {
    id: root

    required property var efforts
    property string selectedEffort
    readonly property bool selectedEffortIsListed: selectedValueIsListed

    signal effortSelected(string effort)

    function effortLabel(effort) {
        if (effort === "xhigh")
            return qsTr("Extra high");
        if (effort.length === 0)
            return "";
        return effort.charAt(0).toUpperCase() + effort.slice(1);
    }

    selectedValue: selectedEffort
    model: efforts
    textRole: "reasoningEffort"
    valueRole: "reasoningEffort"
    optionText: value => root.effortLabel(value)
    displayText: {
        if (selectedEffort.length > 0)
            return effortLabel(selectedEffort);
        return count > 0 ? qsTr("Current effort") : qsTr("No efforts available");
    }
    Accessible.name: qsTr("Conversation reasoning effort")
    onValueSelected: value => root.effortSelected(value)

    toolTipText: {
        if (selectedEffortIsListed) {
            const option = efforts[currentIndex];
            if (option && option.description)
                return option.description;
        }
        return qsTr("The selected effort is applied when the next turn starts and remains active for later turns.");
    }
}
