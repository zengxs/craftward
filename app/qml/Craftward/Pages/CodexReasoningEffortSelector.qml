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
            return /*% "Extra high" */ qsTrId("craftward.codex.reasoning_effort.extra_high");
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
        return count > 0 ? /*% "Current effort" */ qsTrId("craftward.codex.reasoning_effort.current") : /*% "No efforts available" */ qsTrId("craftward.codex.reasoning_effort.empty");
    }
    Accessible.name: /*% "Conversation reasoning effort" */ qsTrId("craftward.codex.reasoning_effort.accessible_name")
    onValueSelected: value => root.effortSelected(value)

    toolTipText: {
        if (selectedEffortIsListed) {
            const option = efforts[currentIndex];
            if (option && option.description)
                return option.description;
        }
        return /*% "The selected effort is applied when the next turn starts and remains active for later turns." */ qsTrId("craftward.codex.reasoning_effort.selection_description");
    }
}
