// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite
    width: 640
    height: 480
    Pages.ThreadWorkspaceState {
        id: workspace
    }
    Component {
        id: viewerFactory
        Pages.ImageContentView {
            width: suite.width
            height: suite.height
            resource: workspace.activeTab ? workspace.activeTab.resource : null
            viewState: workspace.activeTab ? workspace.activeTab.viewState : null
        }
    }
    TestCase {
        name: "ImageContentView"
        when: windowShown
        property var viewer
        function cleanup() {
            if (viewer)
                viewer.destroy();
            viewer = null;
            wait(0);
            while (workspace.tabs.length)
                workspace.closeTab(1);
        }
        function openImage(tag) {
            workspace.openImage({
                resourceId: "image:" + tag,
                url: Qt.resolvedUrl("fixtures/attachment-" + tag + ".png")
            });
        }
        function test_fitZoomPanAndSwitchingPreserveEachImageView() {
            openImage("landscape");
            viewer = createTemporaryObject(viewerFactory, suite);
            const image = findChild(viewer, "imageViewImage");
            const viewport = findChild(viewer, "imageViewViewport");
            tryCompare(image, "status", Image.Ready);
            tryVerify(() => !viewer.restoring);
            verify(image.width <= viewport.width);
            verify(image.height <= viewport.height);
            mouseClick(findChild(viewer, "imageActualSize"));
            tryVerify(() => !viewer.restoring);
            compare(image.width, 1200);
            compare(image.height, 800);
            mouseDrag(viewport, viewport.width / 2, viewport.height / 2, -80, -60);
            viewport.cancelFlick();
            verify(viewport.contentX > 0);
            const state = workspace.activeTab.viewState;
            const x = viewport.contentX;
            const y = viewport.contentY;
            openImage("portrait");
            tryCompare(image, "status", Image.Ready);
            tryVerify(() => !viewer.restoring && image.implicitWidth === 400);
            verify(viewer.viewState.fit);
            openImage("landscape");
            tryVerify(() => image.status === Image.Ready && image.implicitWidth === 1200 && !viewer.restoring);
            compare(viewer.viewState, state);
            compare(viewer.imageScale, 1);
            verify(Math.abs(viewport.contentX - x) < 1);
            verify(Math.abs(viewport.contentY - y) < 1);
            viewer.destroy();
            viewer = null;
            wait(0);
            viewer = createTemporaryObject(viewerFactory, suite);
            const restoredViewport = findChild(viewer, "imageViewViewport");
            tryVerify(() => !viewer.restoring);
            verify(Math.abs(restoredViewport.contentX - x) < 1);
            verify(Math.abs(restoredViewport.contentY - y) < 1);
            mouseClick(findChild(viewer, "imageFit"));
            tryVerify(() => !viewer.restoring);
            compare(restoredViewport.contentX, 0);
            compare(restoredViewport.contentY, 0);
        }
    }
}
