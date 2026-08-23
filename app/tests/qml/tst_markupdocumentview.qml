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
        id: testDocumentModel
    }

    Component {
        id: viewComponent

        MarkupDocumentView {
            width: 600
            documentModel: testDocumentModel
        }
    }

    TestCase {
        name: "MarkupDocumentView"

        function init() {
            testDocumentModel.clear();
            testDocumentModel.append({
                "blockId": "prose:0",
                "codeBlock": false,
                "blockText": "# Heading",
                "plainText": "Heading",
                "language": "",
                "markdown": true
            });
            testDocumentModel.append({
                "blockId": "code:11",
                "codeBlock": true,
                "blockText": "let answer = 42;\n",
                "plainText": "let answer = 42;\n",
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
            verify(suite.view.implicitWidth >= 560);
        }
    }
}
