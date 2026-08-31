// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import Craftward.Components
import Craftward.Design

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
            ApplicationClipboard.reset();
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
                "segmentText": "let answer = 42;",
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
            const codeSurface = findChild(suite.view, "markupCodeSurface");
            const codeText = findChild(suite.view, "markupCodeText");
            const syntaxLabel = findChild(suite.view, "markupCodeSyntaxLabel");
            verify(codeSurface !== null);
            verify(codeText !== null);
            verify(syntaxLabel !== null);
            compare(codeSurface.color, Theme.dark ? TailwindColors.zinc900 : TailwindColors.zinc50);
            verify(codeSurface.border.width > 0 && codeSurface.border.width <= 1);
            compare(codeSurface.implicitHeight, codeText.implicitHeight + 16);
            compare(syntaxLabel.text, "JavaScript");
        }

        function test_titleCasesAnUnknownSyntaxName() {
            testRenderModel.setProperty(1, "language", "custom_syntax");
            const syntaxLabel = findChild(suite.view, "markupCodeSyntaxLabel");
            verify(syntaxLabel !== null);
            tryCompare(syntaxLabel, "text", "Custom Syntax");
        }

        function test_revealsCopyActionAndCopiesDisplayedCode() {
            const codeText = findChild(suite.view, "markupCodeText");
            const copyButton = findChild(suite.view, "markupCodeCopyButton");
            verify(codeText !== null);
            verify(copyButton !== null);
            compare(copyButton.toolTipText, "Copy");
            codeText.forceActiveFocus();
            tryVerify(() => copyButton.visible);
            copyButton.forceActiveFocus();
            tryVerify(() => copyButton.activeFocus);
            keyClick(Qt.Key_Space);
            compare(ApplicationClipboard.lastCopiedText, "let answer = 42;");
            compare(ApplicationClipboard.copyCount, 1);
            compare(copyButton.toolTipText, "Copied");
        }
    }
}
