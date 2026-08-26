// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import Craftward.Components

Control {
    id: root

    required property var pageModel
    // The delegate contract exposes sourceRow, dataRevision, entryId, and presentationVisible.
    required property Component rowDelegate
    property real bottomContentInset: 64
    property real contentHorizontalInset: 20
    property real contentMaximumWidth: 920
    property real estimatedPageHeight: 720
    property var pageHeights: ({})
    property int pageHeightRevision: 0
    property int knownPageCount: 0
    property bool followLiveTail: true
    property var lastVisibleAnchor: null
    property var pendingAnchor: null
    property int pendingAnchorPasses: 0
    property real lastObservedContentY: 0
    property bool adjustingAnchor: false
    property int heightCompensationBeforePage: -1
    property int lastScrollDirection: 0
    property bool completed: false
    readonly property int pageCount: pageModel ? Math.max(0, Number(pageModel.pageCount)) : 0
    readonly property int modelRevision: pageModel ? Number(pageModel.revision) : 0
    readonly property real contentColumnWidth: Math.max(0, Math.min(availableWidth - contentHorizontalInset * 2, contentMaximumWidth))
    readonly property int activeFirstPage: pageWindow.firstPage
    readonly property int activeLastPage: pageWindow.lastPage
    readonly property int loadedPageCount: activePagesModel.count
    readonly property real leadingPlaceholderHeight: leadingPlaceholder.height
    readonly property real trailingPlaceholderHeight: trailingPlaceholder.height
    readonly property real contentY: scrollViewport.contentY
    readonly property real scrollContentHeight: scrollViewport.contentHeight
    readonly property real viewportHeight: scrollViewport.height

    function pageIdAt(pageIndex) {
        const currentRevision = root.modelRevision;
        return root.pageModel && currentRevision >= 0 ? String(root.pageModel.pageId(pageIndex)) : "";
    }

    function pageFirstRowAt(pageIndex) {
        const currentRevision = root.modelRevision;
        return root.pageModel && currentRevision >= 0 ? Number(root.pageModel.pageFirstRow(pageIndex)) : -1;
    }

    function pageRowCountAt(pageIndex) {
        const currentRevision = root.modelRevision;
        return root.pageModel && currentRevision >= 0 ? Number(root.pageModel.pageRowCount(pageIndex)) : 0;
    }

    function cachedPageHeight(pageId) {
        const currentRevision = root.pageHeightRevision;
        const cached = currentRevision >= 0 ? Number(root.pageHeights[String(pageId)]) : 0;
        return Number.isFinite(cached) && cached > 0 ? cached : 0;
    }

    function pageHeightAt(pageIndex) {
        const cached = root.cachedPageHeight(root.pageIdAt(pageIndex));
        return cached > 0 ? cached : Math.max(1, root.estimatedPageHeight);
    }

    function aggregatePageHeight(firstPage, lastPage) {
        const currentRevision = root.pageHeightRevision;
        if (currentRevision < 0 || firstPage < 0 || lastPage < firstPage)
            return 0;
        const boundedLast = Math.min(root.pageCount - 1, lastPage);
        let height = 0;
        for (let page = firstPage; page <= boundedLast; ++page)
            height += root.pageHeightAt(page);
        return height;
    }

    function placeholderHeight(firstPage, lastPage) {
        if (firstPage < 0 || lastPage < firstPage)
            return 0;
        return Math.max(scrollViewport.height, root.aggregatePageHeight(firstPage, lastPage));
    }

    function invokeDelegate(object, methodName, argument) {
        const method = object ? object[methodName] : null;
        return typeof method === "function" ? method.call(object, argument) : null;
    }

    function delegateProperty(object, propertyName, fallback) {
        if (!object)
            return fallback;
        const value = object[propertyName];
        return value === undefined ? fallback : value;
    }

    function rememberPageHeight(pageId, measuredHeight) {
        const key = String(pageId);
        const height = Math.ceil(Number(measuredHeight));
        if (!key || !Number.isFinite(height) || height <= 0 || root.pageHeights[key] === height)
            return;
        root.pageHeights[key] = height;
        ++root.pageHeightRevision;
    }

    function clearPageHeights() {
        root.pageHeights = {};
        ++root.pageHeightRevision;
    }

    function synchronizeActivePages() {
        const first = root.activeFirstPage;
        const last = root.activeLastPage;
        if (first < 0 || last < first) {
            activePagesModel.clear();
            return;
        }

        while (activePagesModel.count > 0 && Number(activePagesModel.get(0).pageIndex) < first)
            activePagesModel.remove(0);
        while (activePagesModel.count > 0 && Number(activePagesModel.get(activePagesModel.count - 1).pageIndex) > last)
            activePagesModel.remove(activePagesModel.count - 1);

        if (activePagesModel.count === 0) {
            for (let page = first; page <= last; ++page)
                activePagesModel.append({
                    pageIndex: page,
                    stableId: root.pageIdAt(page),
                    firstSourceRow: root.pageFirstRowAt(page),
                    sourceRowCount: root.pageRowCountAt(page)
                });
        } else {
            while (Number(activePagesModel.get(0).pageIndex) > first) {
                const page = Number(activePagesModel.get(0).pageIndex) - 1;
                activePagesModel.insert(0, {
                    pageIndex: page,
                    stableId: root.pageIdAt(page),
                    firstSourceRow: root.pageFirstRowAt(page),
                    sourceRowCount: root.pageRowCountAt(page)
                });
            }
            while (Number(activePagesModel.get(activePagesModel.count - 1).pageIndex) < last) {
                const page = Number(activePagesModel.get(activePagesModel.count - 1).pageIndex) + 1;
                activePagesModel.append({
                    pageIndex: page,
                    stableId: root.pageIdAt(page),
                    firstSourceRow: root.pageFirstRowAt(page),
                    sourceRowCount: root.pageRowCountAt(page)
                });
            }
        }

        for (let row = 0; row < activePagesModel.count; ++row) {
            const page = Number(activePagesModel.get(row).pageIndex);
            activePagesModel.set(row, {
                pageIndex: page,
                stableId: root.pageIdAt(page),
                firstSourceRow: root.pageFirstRowAt(page),
                sourceRowCount: root.pageRowCountAt(page)
            });
        }
    }

    function resetWindowToLatest() {
        root.knownPageCount = root.pageCount;
        pageWindow.resetToLatest();
        root.synchronizeActivePages();
        root.followLiveTail = true;
        Qt.callLater(root.scrollToBottom);
    }

    function resetForNewContent() {
        windowCompactionTimer.stop();
        overscanAdvanceCooldown.stop();
        heightCompensationTimer.stop();
        anchorRestoreTimer.stop();
        root.pendingAnchor = null;
        root.heightCompensationBeforePage = -1;
        root.clearPageHeights();
        root.followLiveTail = true;
        root.resetWindowToLatest();
    }

    function synchronizePageWindow() {
        const count = root.pageCount;
        if (count <= 0) {
            root.knownPageCount = 0;
            pageWindow.clampToPageCount();
            root.synchronizeActivePages();
            return;
        }

        if (root.activeFirstPage < 0 || count < root.knownPageCount) {
            root.resetWindowToLatest();
            return;
        }

        if (root.followLiveTail) {
            pageWindow.resetToLatest();
            root.synchronizeActivePages();
            Qt.callLater(root.scrollToBottom);
        } else {
            pageWindow.clampToPageCount();
            root.synchronizeActivePages();
        }
        root.knownPageCount = count;
    }

    function captureVisibleAnchor() {
        for (let row = 0; row < pageRepeater.count; ++row) {
            const page = pageRepeater.itemAt(row);
            if (!page)
                continue;
            const anchor = root.invokeDelegate(page, "captureVisibleAnchor", scrollViewport.contentY);
            if (anchor)
                return anchor;
        }
        return null;
    }

    function delegateForEntry(entryId) {
        for (let row = 0; row < pageRepeater.count; ++row) {
            const page = pageRepeater.itemAt(row);
            if (!page)
                continue;
            const delegate = root.invokeDelegate(page, "delegateForEntry", entryId);
            if (delegate)
                return delegate;
        }
        return null;
    }

    function scheduleAnchorRestore(anchor) {
        if (!anchor)
            return;
        root.pendingAnchor = anchor;
        root.pendingAnchorPasses = 0;
        anchorRestoreTimer.restart();
    }

    function restorePendingAnchor() {
        if (!root.pendingAnchor)
            return false;
        const item = root.delegateForEntry(root.pendingAnchor.entryId);
        if (!item)
            return false;
        const top = item.mapToItem(scrollViewport.contentItem, 0, 0).y;
        const minimumY = scrollViewport.originY;
        const maximumY = Math.max(minimumY, minimumY + scrollViewport.contentHeight - scrollViewport.height);
        root.adjustingAnchor = true;
        scrollViewport.contentY = Math.max(minimumY, Math.min(maximumY, top - root.pendingAnchor.offset));
        root.lastObservedContentY = scrollViewport.contentY;
        root.adjustingAnchor = false;
        return true;
    }

    function activePageTop() {
        const page = pageRepeater.itemAt(0);
        return page ? page.mapToItem(scrollViewport.contentItem, 0, 0).y : leadingPlaceholder.height;
    }

    function activePageBottom() {
        const page = pageRepeater.itemAt(pageRepeater.count - 1);
        return page ? page.mapToItem(scrollViewport.contentItem, 0, page.height).y : leadingPlaceholder.height;
    }

    function expandWindow(direction) {
        if (direction === 0 || pageWindow.activePageCount >= pageWindow.maximumWindowSize)
            return false;
        if (direction < 0 && root.activeFirstPage <= 0)
            return false;
        if (direction > 0 && root.activeLastPage >= root.pageCount - 1)
            return false;
        const previousFirstPage = root.activeFirstPage;
        if (!pageWindow.expand(direction))
            return false;
        if (direction < 0)
            root.heightCompensationBeforePage = previousFirstPage;
        root.synchronizeActivePages();
        root.followLiveTail = false;
        if (direction < 0)
            heightCompensationTimer.restart();
        return true;
    }

    function advanceWindow(direction) {
        if (direction === 0)
            return false;
        if (direction < 0 && root.activeFirstPage <= 0)
            return false;
        if (direction > 0 && root.activeLastPage >= root.pageCount - 1)
            return false;
        const previousFirstPage = root.activeFirstPage;
        if (!pageWindow.advance(direction))
            return false;
        if (direction < 0)
            root.heightCompensationBeforePage = previousFirstPage;
        root.synchronizeActivePages();
        root.followLiveTail = false;
        if (direction < 0)
            heightCompensationTimer.restart();
        return true;
    }

    function pageAtContentY(contentY) {
        let remaining = Math.max(0, Number(contentY) - timelineColumn.y);
        for (let page = 0; page < root.pageCount; ++page) {
            const height = root.pageHeightAt(page);
            if (remaining < height)
                return page;
            remaining -= height;
        }
        return root.pageCount - 1;
    }

    function catchUpWindow(targetPage) {
        if (targetPage < 0 || targetPage >= root.pageCount)
            return false;
        if (!pageWindow.setWindowAround(targetPage, pageWindow.maximumWindowSize))
            return false;
        root.heightCompensationBeforePage = targetPage;
        root.synchronizeActivePages();
        root.followLiveTail = false;
        heightCompensationTimer.restart();
        return true;
    }

    function compensateForPageHeightChange(pageIndex, heightDelta) {
        if (root.heightCompensationBeforePage < 0 || pageIndex >= root.heightCompensationBeforePage || Math.abs(heightDelta) < 0.5)
            return;
        root.adjustingAnchor = true;
        scrollViewport.contentY += heightDelta;
        root.lastObservedContentY = scrollViewport.contentY;
        root.adjustingAnchor = false;
        heightCompensationTimer.restart();
    }

    function lookaheadDistance() {
        const viewportHeight = Math.max(1, scrollViewport.height);
        const velocityDistance = Math.min(viewportHeight * 1.5, Math.abs(scrollViewport.verticalVelocity) * 0.2);
        return viewportHeight * 1.5 + velocityDistance;
    }

    function prepareOverscan(direction) {
        if (direction === 0 || root.activeFirstPage < 0)
            return;
        root.lastScrollDirection = direction;

        const targetPage = root.pageAtContentY(scrollViewport.contentY + scrollViewport.height / 2);
        if (targetPage < root.activeFirstPage - 1 || targetPage > root.activeLastPage + 1) {
            if (root.catchUpWindow(targetPage))
                overscanAdvanceCooldown.restart();
            return;
        }

        if (pageWindow.activePageCount < pageWindow.maximumWindowSize) {
            if (root.expandWindow(direction))
                overscanAdvanceCooldown.restart();
            return;
        }
        if (overscanAdvanceCooldown.running)
            return;

        const lookahead = root.lookaheadDistance();
        if (direction < 0 && root.activeFirstPage > 0 && scrollViewport.contentY - root.activePageTop() <= lookahead) {
            if (root.advanceWindow(direction))
                overscanAdvanceCooldown.restart();
        } else if (direction > 0 && root.activeLastPage < root.pageCount - 1 && root.activePageBottom() - (scrollViewport.contentY + scrollViewport.height) <= lookahead) {
            if (root.advanceWindow(direction))
                overscanAdvanceCooldown.restart();
        }
    }

    function pageAtViewportCenter() {
        const centerY = scrollViewport.contentY + scrollViewport.height / 2;
        for (let row = 0; row < pageRepeater.count; ++row) {
            const page = pageRepeater.itemAt(row);
            if (!page || page.height <= 0)
                continue;
            const top = page.mapToItem(scrollViewport.contentItem, 0, 0).y;
            if (centerY >= top && centerY <= top + page.height)
                return Number(activePagesModel.get(row).pageIndex);
        }
        return root.pageAtContentY(centerY);
    }

    function compactWindow() {
        if (scrollViewport.moving || pageWindow.activePageCount <= pageWindow.baseWindowSize)
            return false;
        const focusPage = root.pageAtViewportCenter();
        if (focusPage < 0)
            return false;
        const anchor = root.captureVisibleAnchor() ?? root.lastVisibleAnchor;
        if (!pageWindow.compactAround(focusPage))
            return false;
        root.synchronizeActivePages();
        root.scheduleAnchorRestore(anchor);
        return true;
    }

    function scrollToBottom() {
        const minimumY = scrollViewport.originY;
        scrollViewport.contentY = Math.max(minimumY, minimumY + scrollViewport.contentHeight - scrollViewport.height);
        root.lastObservedContentY = scrollViewport.contentY;
        root.lastVisibleAnchor = root.captureVisibleAnchor();
    }

    function positionAtContentY(contentY) {
        const minimumY = scrollViewport.originY;
        const maximumY = Math.max(minimumY, minimumY + scrollViewport.contentHeight - scrollViewport.height);
        scrollViewport.contentY = Math.max(minimumY, Math.min(maximumY, Number(contentY)));
    }

    function followLatest() {
        root.followLiveTail = true;
        pageWindow.resetToLatest();
        root.synchronizeActivePages();
        Qt.callLater(root.scrollToBottom);
    }

    padding: 0

    ListModel {
        id: activePagesModel
    }

    CodexTimelinePageWindow {
        id: pageWindow

        pageCount: root.pageCount
    }

    contentItem: Flickable {
        id: scrollViewport

        objectName: "codexTimelineScrollViewport"
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        contentWidth: width
        contentHeight: timelineColumn.y + timelineColumn.height + root.bottomContentInset
        flickableDirection: Flickable.VerticalFlick
        ScrollBar.vertical: OverlayScrollBar {}

        onContentHeightChanged: {
            if (root.followLiveTail)
                Qt.callLater(root.scrollToBottom);
        }
        onMovementStarted: {
            windowCompactionTimer.stop();
            anchorRestoreTimer.stop();
            root.pendingAnchor = null;
            root.followLiveTail = false;
            root.lastObservedContentY = contentY;
            root.lastVisibleAnchor = root.captureVisibleAnchor();
        }
        onContentYChanged: {
            const delta = contentY - root.lastObservedContentY;
            root.lastObservedContentY = contentY;
            if (root.adjustingAnchor || Math.abs(delta) < 0.5)
                return;
            root.prepareOverscan(delta < 0 ? -1 : 1);
        }
        onMovementEnded: {
            root.followLiveTail = root.activeLastPage === root.pageCount - 1 && atYEnd;
            root.lastVisibleAnchor = root.captureVisibleAnchor();
            windowCompactionTimer.restart();
        }

        Column {
            id: timelineColumn

            x: Math.round((scrollViewport.width - width) / 2)
            y: 20
            width: root.contentColumnWidth
            spacing: 0

            Item {
                id: leadingPlaceholder

                width: timelineColumn.width
                height: root.activeFirstPage > 0 ? root.placeholderHeight(0, root.activeFirstPage - 1) : 0

                BusyIndicator {
                    anchors {
                        horizontalCenter: parent.horizontalCenter
                        bottom: parent.bottom
                        bottomMargin: 16
                    }
                    width: 18
                    height: 18
                    visible: parent.height > 0 && scrollViewport.contentY <= parent.height + scrollViewport.height
                    running: visible
                }
            }

            Repeater {
                id: pageRepeater

                model: activePagesModel

                delegate: Item {
                    id: pageDelegate

                    required property int pageIndex
                    required property string stableId
                    required property int firstSourceRow
                    required property int sourceRowCount
                    readonly property real retainedHeight: root.pageHeightAt(pageIndex)
                    property real lastEffectiveHeight: retainedHeight

                    function captureVisibleAnchor(contentY) {
                        return pageContent.captureVisibleAnchor(contentY);
                    }

                    function delegateForEntry(entryId) {
                        return pageContent.delegateForEntry(entryId);
                    }

                    width: timelineColumn.width
                    height: pageContent.implicitHeight > 0 ? pageContent.implicitHeight : retainedHeight
                    onHeightChanged: {
                        const delta = height - lastEffectiveHeight;
                        lastEffectiveHeight = height;
                        root.compensateForPageHeightChange(pageIndex, delta);
                    }

                    Column {
                        id: pageContent

                        function captureVisibleAnchor(contentY) {
                            for (let row = 0; row < rowRepeater.count; ++row) {
                                const slot = rowRepeater.itemAt(row);
                                const item = root.delegateProperty(slot, "item", null);
                                if (!item || !item.visible || item.height <= 0)
                                    continue;
                                const top = item.mapToItem(scrollViewport.contentItem, 0, 0).y;
                                if (top + item.height >= contentY)
                                    return {
                                        entryId: item.entryId,
                                        offset: top - contentY
                                    };
                            }
                            return null;
                        }

                        function delegateForEntry(entryId) {
                            for (let row = 0; row < rowRepeater.count; ++row) {
                                const slot = rowRepeater.itemAt(row);
                                const item = root.delegateProperty(slot, "item", null);
                                if (item && item.visible && item.entryId === entryId)
                                    return item;
                            }
                            return null;
                        }

                        width: parent.width
                        spacing: 10
                        onImplicitHeightChanged: root.rememberPageHeight(pageDelegate.stableId, implicitHeight)

                        Repeater {
                            id: rowRepeater

                            model: pageDelegate.sourceRowCount

                            delegate: Loader {
                                id: rowLoader

                                required property int index
                                property int sourceRow: pageDelegate.firstSourceRow + index
                                property int dataRevision: root.modelRevision

                                function synchronizeItem() {
                                    if (!item)
                                        return;
                                    item["sourceRow"] = sourceRow;
                                    item["dataRevision"] = dataRevision;
                                }

                                width: pageContent.width
                                visible: Boolean(root.delegateProperty(item, "presentationVisible", true))
                                sourceComponent: root.rowDelegate
                                onDataRevisionChanged: synchronizeItem()
                                onLoaded: synchronizeItem()
                                onSourceRowChanged: synchronizeItem()
                            }
                        }

                        Item {
                            width: 1
                            height: pageDelegate.sourceRowCount > 0 ? 10 : 0
                        }

                        Component.onCompleted: Qt.callLater(() => root.rememberPageHeight(pageDelegate.stableId, implicitHeight))
                    }
                }
            }

            Item {
                id: trailingPlaceholder

                width: timelineColumn.width
                height: root.activeLastPage >= 0 && root.activeLastPage < root.pageCount - 1 ? root.placeholderHeight(root.activeLastPage + 1, root.pageCount - 1) : 0

                BusyIndicator {
                    anchors {
                        horizontalCenter: parent.horizontalCenter
                        top: parent.top
                        topMargin: 16
                    }
                    width: 18
                    height: 18
                    visible: parent.height > 0 && scrollViewport.contentY + scrollViewport.height >= parent.y - scrollViewport.height
                    running: visible
                }
            }
        }
    }

    Connections {
        target: root.pageModel
        ignoreUnknownSignals: true

        function onStatisticsChanged() {
            root.synchronizePageWindow();
        }

        function onRevisionChanged() {
            root.synchronizeActivePages();
        }
    }

    Timer {
        id: anchorRestoreTimer

        interval: 16
        repeat: true
        onTriggered: {
            ++root.pendingAnchorPasses;
            root.restorePendingAnchor();
            if (root.pendingAnchorPasses >= 12) {
                stop();
                root.lastVisibleAnchor = root.captureVisibleAnchor();
                root.pendingAnchor = null;
            }
        }
    }

    Timer {
        id: overscanAdvanceCooldown

        interval: 48
        onTriggered: {
            if (root.lastScrollDirection !== 0)
                root.prepareOverscan(root.lastScrollDirection);
        }
    }

    Timer {
        id: heightCompensationTimer

        interval: 750
        onTriggered: root.heightCompensationBeforePage = -1
    }

    Timer {
        id: windowCompactionTimer

        interval: 250
        onTriggered: root.compactWindow()
    }

    onPageCountChanged: root.synchronizePageWindow()
    onModelRevisionChanged: root.synchronizeActivePages()
    onContentColumnWidthChanged: {
        if (!root.completed)
            return;
        const anchor = root.captureVisibleAnchor() ?? root.lastVisibleAnchor;
        root.clearPageHeights();
        root.scheduleAnchorRestore(anchor);
    }

    Component.onCompleted: {
        root.completed = true;
        root.resetWindowToLatest();
    }
}
