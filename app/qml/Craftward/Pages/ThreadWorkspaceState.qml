// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    id: root

    property string threadId: ""
    property var tabs: []
    property int activeIndex: 0
    property var sessions: ({})
    property bool filesExpanded: true
    readonly property var activeTab: activeIndex > 0 && activeIndex <= tabs.length ? tabs[activeIndex - 1] : null

    function selectThread(id) {
        if (threadId === id)
            return;
        sessions[threadId] = {
            tabs: tabs.slice(),
            activeIndex: activeIndex,
            filesExpanded: filesExpanded
        };
        threadId = id;
        const saved = sessions[id];
        tabs = saved ? saved.tabs.slice() : [];
        activeIndex = saved ? Math.min(saved.activeIndex, tabs.length) : 0;
        filesExpanded = saved ? saved.filesExpanded : true;
    }

    function openTab(tab) {
        const index = tabs.findIndex(item => item.id === tab.id);
        const updated = tabs.slice();
        if (index >= 0) {
            updated[index] = tab;
            tabs = updated;
            activeIndex = index + 1;
        } else {
            updated.push(tab);
            tabs = updated;
            activeIndex = tabs.length;
        }
    }

    function closeTab(index) {
        if (index <= 0 || index > tabs.length)
            return;
        const updated = tabs.slice();
        updated.splice(index - 1, 1);
        tabs = updated;
        if (activeIndex >= index)
            activeIndex = Math.max(0, activeIndex - 1);
    }

    function moveTab(from, to) {
        if (from <= 0 || to <= 0 || from > tabs.length || to > tabs.length || from === to)
            return;
        const activeId = activeTab ? activeTab.id : "";
        const updated = tabs.slice();
        updated.splice(to - 1, 0, updated.splice(from - 1, 1)[0]);
        tabs = updated;
        activeIndex = activeId.length > 0 ? tabs.findIndex(tab => tab.id === activeId) + 1 : 0;
    }
}
