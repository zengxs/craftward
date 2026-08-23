// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick

TableView {
    id: root

    required property bool loading
    required property string layoutKey
    required property var rowKeyProvider
    property real estimatedRowHeight: 160
    property bool followLiveTail: true
    property var _heightCache: ({})
    property var _rowHeights: []
    property var _rowKeys: []
    property real _rowHeightSum: 0
    property real _publishedContentHeight: 0
    property bool _completed: false
    property bool _endScheduled: false
    property bool _layoutScheduled: false
    property bool _programmaticMove: false
    property var _pendingHeightMeasurements: ({})
    property var _pendingHeightChanges: []
    property var _pendingLayoutAnchor: null

    function _normalizedRowHeight(value) {
        return Number.isFinite(value) && value > 0 ? Math.max(1, Math.ceil(value)) : 160;
    }

    function _rowHeightAt(row) {
        if (row < 0 || row >= _rowHeights.length)
            return _normalizedRowHeight(estimatedRowHeight);
        return _rowHeights[row];
    }

    function _rowKeyAt(row) {
        if (typeof rowKeyProvider !== "function")
            return "";
        const value = rowKeyProvider(row);
        return value === undefined || value === null ? "" : String(value);
    }

    function _cachedRowHeight(rowKey) {
        if (rowKey.length === 0)
            return 0;
        const value = _heightCache["entry:" + rowKey];
        return Number.isFinite(value) && value > 0 ? value : 0;
    }

    function _clearMeasurementCache() {
        _heightCache = {};
    }

    function _maximumContentY() {
        return originY + Math.max(0, contentHeight - height);
    }

    function _boundedContentY(value) {
        return Math.max(originY, Math.min(_maximumContentY(), value));
    }

    function _captureVisibleAnchor() {
        if (followLiveTail || topRow < 0 || topRow >= rows)
            return null;

        let row = topRow;
        let item = itemAtCell(Qt.point(0, row));
        if (!item)
            return null;
        let visualY = item.mapToItem(root, 0, 0).y;
        if (visualY < -0.5 && row + 1 < rows) {
            const nextItem = itemAtCell(Qt.point(0, row + 1));
            if (nextItem) {
                row += 1;
                item = nextItem;
                visualY = item.mapToItem(root, 0, 0).y;
            }
        }
        return {
            "row": row,
            "rowKey": _rowKeyAt(row),
            "visualY": visualY
        };
    }

    function _restoreVisibleAnchor(anchor) {
        if (!anchor || anchor.row < 0 || anchor.row >= rows || _rowKeyAt(anchor.row) !== anchor.rowKey)
            return false;
        const item = itemAtCell(Qt.point(0, anchor.row));
        if (!item)
            return false;
        const delta = item.mapToItem(root, 0, 0).y - anchor.visualY;
        if (Math.abs(delta) < 0.5)
            return true;
        _programmaticMove = true;
        contentY = _boundedContentY(contentY + delta);
        Qt.callLater(_finishProgrammaticMove);
        return true;
    }

    function _finishProgrammaticMove() {
        if (moving) {
            programmaticMoveTimer.restart();
            return;
        }
        _programmaticMove = false;
        heightCommitTimer.restart();
    }

    function _userScrollActive() {
        return moving && !_programmaticMove;
    }

    function _applyScheduledLayout() {
        if (!_layoutScheduled)
            return;
        if (_userScrollActive()) {
            _pendingLayoutAnchor = null;
            return;
        }
        _layoutScheduled = false;
        const anchor = _pendingLayoutAnchor || _captureVisibleAnchor();
        _pendingLayoutAnchor = null;
        let fallbackDelta = 0;
        if (anchor) {
            for (const change of _pendingHeightChanges) {
                if (change.row < anchor.row)
                    fallbackDelta += change.delta;
            }
        }
        _pendingHeightChanges = [];
        const wasFlicking = flicking;
        const remainingVelocity = verticalVelocity;
        forceLayout();
        _publishedContentHeight = _rowHeightSum + Math.max(0, rows - 1) * rowSpacing;
        const anchorRestored = _restoreVisibleAnchor(anchor);
        const anchorStillMatches = anchor && anchor.row >= 0 && anchor.row < rows && _rowKeyAt(anchor.row) === anchor.rowKey;
        if (!anchorRestored && anchorStillMatches && Math.abs(fallbackDelta) >= 0.5) {
            _programmaticMove = true;
            contentY = _boundedContentY(contentY + fallbackDelta);
            Qt.callLater(_finishProgrammaticMove);
        }
        if (wasFlicking && Math.abs(remainingVelocity) >= 1) {
            _programmaticMove = true;
            flick(0, -remainingVelocity);
            Qt.callLater(_finishProgrammaticMove);
        }
    }

    function _scheduleLayout(deferToFrame) {
        if (_layoutScheduled)
            return;
        _layoutScheduled = true;
        // Coalesce resize-driven layout requests to at most one pass per frame.
        if (deferToFrame)
            layoutTimer.start();
        else
            Qt.callLater(_applyScheduledLayout);
    }

    function _synchronizeRows(resetMeasurements) {
        if (!_completed)
            return;

        if (resetMeasurements)
            _pendingHeightMeasurements = {};

        const previousHeights = _rowHeights;
        const previousKeys = _rowKeys;
        let retainedKeysMatch = !resetMeasurements && rows >= previousHeights.length && previousKeys.length === previousHeights.length;
        for (let row = 0; retainedKeysMatch && row < previousKeys.length; ++row)
            retainedKeysMatch = previousKeys[row] === _rowKeyAt(row);

        if (retainedKeysMatch) {
            const nextHeights = previousHeights.slice();
            const nextKeys = previousKeys.slice();
            let nextHeightSum = _rowHeightSum;
            for (let row = previousHeights.length; row < rows; ++row) {
                const rowKey = _rowKeyAt(row);
                const cachedHeight = _cachedRowHeight(rowKey);
                const height = cachedHeight > 0 ? cachedHeight : _normalizedRowHeight(estimatedRowHeight);
                nextHeights.push(height);
                nextKeys.push(rowKey);
                nextHeightSum += height;
                setRowHeight(row, height);
            }
            _rowHeights = nextHeights;
            _rowKeys = nextKeys;
            _rowHeightSum = nextHeightSum;
            _scheduleLayout();
            if (followLiveTail)
                _scheduleEndPosition();
            return;
        }

        _pendingHeightChanges = [];
        const nextHeights = [];
        const nextKeys = [];
        let nextHeightSum = 0;

        for (let row = 0; row < rows; ++row) {
            const rowKey = _rowKeyAt(row);
            const retainMeasurement = !resetMeasurements && row < previousHeights.length && row < previousKeys.length && previousKeys[row] === rowKey;
            const cachedHeight = _cachedRowHeight(rowKey);
            const height = retainMeasurement ? previousHeights[row] : (cachedHeight > 0 ? cachedHeight : _normalizedRowHeight(estimatedRowHeight));
            nextHeights.push(height);
            nextKeys.push(rowKey);
            nextHeightSum += height;
        }

        _rowHeights = nextHeights;
        _rowKeys = nextKeys;
        _rowHeightSum = nextHeightSum;

        clearRowHeights();
        for (let row = 0; row < nextHeights.length; ++row)
            setRowHeight(row, nextHeights[row]);
        _scheduleLayout();

        if (followLiveTail)
            _scheduleEndPosition();
    }

    function _commitPendingHeightMeasurements() {
        if (_userScrollActive())
            return;

        const measurements = _pendingHeightMeasurements;
        _pendingHeightMeasurements = {};
        const measurementRows = Object.keys(measurements);
        if (measurementRows.length === 0)
            return;

        if (_rowHeights.length !== rows)
            _synchronizeRows(false);

        let changed = false;
        const anchor = _captureVisibleAnchor();
        for (const rowKey of measurementRows) {
            const measurement = measurements[rowKey];
            const row = measurement.row;
            if (row < 0 || row >= rows || row >= _rowHeights.length || _rowKeyAt(row) !== measurement.entryId)
                continue;

            const previousHeight = _rowHeights[row];
            if (Math.abs(measurement.height - previousHeight) < 0.5)
                continue;

            const delta = measurement.height - previousHeight;
            _rowKeys[row] = measurement.entryId;
            _rowHeights[row] = measurement.height;
            setRowHeight(row, measurement.height);
            _rowHeightSum += delta;
            _pendingHeightChanges.push({
                "row": row,
                "delta": delta
            });
            changed = true;
        }

        if (!changed)
            return;
        if (!_pendingLayoutAnchor)
            _pendingLayoutAnchor = anchor;
        _scheduleLayout(false);

        if (followLiveTail)
            _scheduleEndPosition();
    }

    function recordRowHeight(row, entryId, height) {
        if (!_completed || row < 0 || row >= rows || typeof entryId !== "string" || entryId.length === 0)
            return;
        if (_rowKeyAt(row) !== entryId)
            return;

        const measuredHeight = _normalizedRowHeight(height);
        _heightCache["entry:" + entryId] = measuredHeight;
        _pendingHeightMeasurements[row] = {
            "row": row,
            "entryId": entryId,
            "height": measuredHeight
        };
        if (!_userScrollActive())
            _commitPendingHeightMeasurements();
    }

    function _applyEndPosition() {
        _endScheduled = false;
        if (!followLiveTail || loading || rows === 0)
            return;

        _programmaticMove = true;
        forceLayout();
        positionViewAtRow(rows - 1, TableView.AlignBottom);
        contentY = _maximumContentY();
        Qt.callLater(_finishProgrammaticMove);
    }

    function _scheduleEndPosition() {
        if (_endScheduled || !followLiveTail || loading)
            return;
        _endScheduled = true;
        Qt.callLater(_applyEndPosition);
    }

    function followLatest() {
        followLiveTail = true;
        _scheduleEndPosition();
    }

    clip: true
    reuseItems: true
    boundsBehavior: Flickable.StopAtBounds
    flickableDirection: Flickable.VerticalFlick
    contentWidth: width
    contentHeight: _publishedContentHeight
    columnWidthProvider: function (column) {
        return column === 0 ? width : 0;
    }
    rowHeightProvider: function (row) {
        return _rowHeightAt(row);
    }

    onRowsChanged: _synchronizeRows(false)
    onRowSpacingChanged: _scheduleLayout()
    onContentHeightChanged: _scheduleEndPosition()
    onLoadingChanged: _scheduleEndPosition()
    onLayoutKeyChanged: {
        followLiveTail = true;
        _clearMeasurementCache();
        _synchronizeRows(true);
        _scheduleEndPosition();
    }
    onEstimatedRowHeightChanged: {
        _clearMeasurementCache();
        _synchronizeRows(true);
    }
    onMovementStarted: {
        if (!_programmaticMove)
            followLiveTail = false;
    }
    onMovementEnded: {
        if (!_programmaticMove)
            followLiveTail = atYEnd;
        heightCommitTimer.restart();
    }

    onWidthChanged: {
        _clearMeasurementCache();
        _pendingHeightMeasurements = {};
        _pendingLayoutAnchor = null;
        _scheduleLayout(true);
    }

    Connections {
        target: root.model
        ignoreUnknownSignals: true

        function onModelReset() {
            Qt.callLater(function () {
                root._synchronizeRows(false);
            });
        }
    }

    Timer {
        id: layoutTimer

        interval: 16
        onTriggered: root._applyScheduledLayout()
    }

    Timer {
        id: heightCommitTimer

        interval: 0
        onTriggered: {
            root._commitPendingHeightMeasurements();
            if (root._layoutScheduled)
                Qt.callLater(root._applyScheduledLayout);
        }
    }

    Timer {
        id: programmaticMoveTimer

        interval: 16
        onTriggered: root._finishProgrammaticMove()
    }

    Component.onCompleted: {
        _completed = true;
        _synchronizeRows(true);
        _scheduleEndPosition();
    }
}
