// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import Craftward.Components

Item {
    id: suite

    width: 640
    height: 480

    property var view

    ListModel {
        id: testRenderModel
    }

    QtObject {
        id: testDocument

        readonly property var renderModel: testRenderModel
    }

    Component {
        id: viewComponent

        MarkupDocumentView {
            width: 600
            documentModel: testDocument
        }
    }

    TestCase {
        name: "MarkupDocumentView"

        function init() {
            testRenderModel.clear();
            testRenderModel.append({
                "segmentId": "prose:0",
                "codeBlock": false,
                "segmentText": "# Heading",
                "language": "",
                "markdown": true
            });
            testRenderModel.append({
                "segmentId": "code:11",
                "codeBlock": true,
                "segmentText": "let answer = 42;\n",
                "language": "javascript",
                "markdown": false
            });
            suite.view = viewComponent.createObject(suite);
            verify(suite.view !== null);
        }

        function cleanup() {
            suite.view.destroy();
            suite.view = null;
        }

        function test_rendersProseAndIndependentCodeBlocks() {
            tryVerify(() => suite.view.implicitHeight > 0);
        }
    }
}
