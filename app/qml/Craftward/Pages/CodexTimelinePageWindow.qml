// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    id: root

    property int pageCount: 0
    readonly property int baseWindowSize: 3
    readonly property int maximumWindowSize: 5
    property int firstPage: -1
    property int lastPage: -1
    readonly property int activePageCount: firstPage < 0 || lastPage < firstPage ? 0 : lastPage - firstPage + 1

    function resetToLatest() {
        if (root.pageCount <= 0) {
            root.firstPage = -1;
            root.lastPage = -1;
            return;
        }

        const size = Math.min(root.baseWindowSize, root.pageCount);
        root.lastPage = root.pageCount - 1;
        root.firstPage = root.lastPage - size + 1;
    }

    function expand(direction) {
        if (direction === 0 || root.activePageCount <= 0 || root.activePageCount >= root.maximumWindowSize)
            return false;

        const oldFirst = root.firstPage;
        const oldLast = root.lastPage;
        if (direction < 0) {
            while (root.activePageCount < root.maximumWindowSize && root.firstPage > 0)
                --root.firstPage;
        } else {
            while (root.activePageCount < root.maximumWindowSize && root.lastPage < root.pageCount - 1)
                ++root.lastPage;
        }
        return root.firstPage !== oldFirst || root.lastPage !== oldLast;
    }

    function advance(direction) {
        if (direction === 0 || root.activePageCount <= 0)
            return false;
        if (root.activePageCount < root.maximumWindowSize)
            return root.expand(direction);

        if (direction < 0 && root.firstPage > 0) {
            --root.firstPage;
            --root.lastPage;
            return true;
        }
        if (direction > 0 && root.lastPage < root.pageCount - 1) {
            ++root.firstPage;
            ++root.lastPage;
            return true;
        }
        return false;
    }

    function setWindowAround(focusPage, requestedSize) {
        if (root.pageCount <= 0) {
            root.firstPage = -1;
            root.lastPage = -1;
            return false;
        }

        const size = Math.min(Math.max(1, Number(requestedSize)), root.pageCount);
        const focus = Math.max(0, Math.min(root.pageCount - 1, Number(focusPage)));
        const maximumFirst = root.pageCount - size;
        const nextFirst = Math.max(0, Math.min(maximumFirst, focus - Math.floor(size / 2)));
        const nextLast = nextFirst + size - 1;
        if (root.firstPage === nextFirst && root.lastPage === nextLast)
            return false;
        root.firstPage = nextFirst;
        root.lastPage = nextLast;
        return true;
    }

    function compactAround(focusPage) {
        return root.setWindowAround(focusPage, root.baseWindowSize);
    }

    function clampToPageCount() {
        if (root.pageCount <= 0) {
            root.firstPage = -1;
            root.lastPage = -1;
            return;
        }
        if (root.firstPage < 0 || root.lastPage < root.firstPage) {
            root.resetToLatest();
            return;
        }

        const size = Math.min(root.activePageCount, root.pageCount);
        root.lastPage = Math.min(root.lastPage, root.pageCount - 1);
        root.firstPage = Math.max(0, root.lastPage - size + 1);
    }

    onPageCountChanged: root.clampToPageCount()
}
