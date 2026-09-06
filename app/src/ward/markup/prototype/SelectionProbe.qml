// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick

// A bounded walkthrough probe against real materialized TextEdits.
Timer {
    id: probe
    required property var host
    interval: 20
    repeat: true
    property int stage: 0
    property int ticks: 0
    property int firstInstance: 0
    property string savedText: ""
    property var checks: []
    readonly property string codeText: 'def greet(name):\n    print("hello " + name)\n\n# Keep the indentation and blank line.\ngreet("world")'
    readonly property string tableText: "Name\tState\tNote\nAlpha a()\tReady\tFirst row [4]\nBeta\tWaiting\t第二行 👋"

    function check(name, passed) {
        checks.push({
            name,
            passed
        });
    }
    function endpoint(nodeId, offset) {
        return {
            nodeId,
            offset
        };
    }
    function nativeCode() {
        const block = host.findBlock("code");
        return block ? block.editor.selectedText.replace(/[\u2028\u2029]/g, "\n") : "";
    }
    function finish() {
        stop();
        prototypeCapture.reportSelection({
            checks,
            snapshot: host.snapshot()
        });
    }
    onTriggered: {
        const selection = host.coordinator;
        if (++ticks > 500) {
            check("walkthrough completed before timeout at stage " + stage, false);
            finish();
            return;
        }
        if (stage === 0) {
            const intro = host.findBlock("intro");
            if (!intro || !host.findBlock("code") || !host.findBlock("cell:8"))
                return;
            firstInstance = intro.instanceNumber;
            selection.begin(endpoint("code:text", 0));
            selection.extend(endpoint("code:text", codeText.length));
            check("code indentation and blank lines", selection.text() === codeText && nativeCode() === codeText);

            selection.begin(endpoint("cell:0:text", 2));
            selection.extend(endpoint("cell:7:text", 4));
            const partialTable = "me\tState\tNote\nAlpha a()\tReady\tFirst row [4]\nBeta\tWait";
            check("partial table cells serialize in row order", selection.text() === partialTable);
            check("partial native cell highlights", host.findBlock("cell:0").editor.selectedText === "me" && host.findBlock("cell:7").editor.selectedText === "Wait" && host.findBlock("cell:8").editor.selectedText === "");
            selection.begin(endpoint("cell:7:text", 4));
            selection.extend(endpoint("cell:0:text", 2));
            check("reverse selection has identical copy order", selection.text() === partialTable && selection.state().backward);

            selection.begin(endpoint("cell:3:text", 3));
            selection.extend(endpoint("cell:3:code", 3));
            check("selection crosses inline styles inside a table cell", selection.text() === "ha a()" && host.findBlock("cell:3").editor.selectedText === "ha a()");
            selection.begin(endpoint("cell:5:annotation", 0));
            selection.extend(endpoint("cell:5:annotation", 3));
            check("table annotation copies its visible label", selection.text() === "[4]" && host.findBlock("cell:5").editor.selectedText === "[4]");

            const tail = intro.nodes[intro.nodes.length - 1].text;
            const emoji = tail.indexOf("👩");
            selection.begin(endpoint("intro:tail", emoji + 1));
            selection.extend(endpoint("intro:tail", emoji + 5));
            check("endpoints preserve a joined emoji", selection.text() === "👩‍💻" && selection.state().anchor.offset === emoji);

            host.exampleSelection();
            savedText = selection.text();
            check("inline through prose, code, and table", savedText.startsWith('print "hello world"') && savedText.includes("\n\n" + codeText + "\n\n") && savedText.endsWith(tableText));
            check("native code highlight matches shared range", nativeCode() === codeText);
            check("table end native highlight", host.findBlock("cell:8").editor.selectedText === "第二行 👋");
            host.jump(28);
            stage = 1;
        } else if (stage === 1) {
            if (host.findBlock("intro") || !host.findBlock("tail:23"))
                return;
            check("selected text items actually destroyed", host.destroyed > 0 && !host.findBlock("code"));
            check("copy survives unmaterialized selection", selection.text() === savedText);
            check("live documents stay below fixture count", host.liveBlocks.length < 24);
            host.jump(0);
            stage = 2;
        } else if (stage === 2) {
            const intro = host.findBlock("intro");
            if (!intro || !host.findBlock("cell:8") || nativeCode() !== codeText)
                return;
            check("recreated text items restore the selection", intro.instanceNumber !== firstInstance && intro.editor.selectedText.startsWith('print "hello world"') && selection.text() === savedText);
            check("recreated table highlights restore", host.findBlock("cell:8").editor.selectedText === "第二行 👋");

            selection.begin(endpoint("intro:lead", 0));
            selection.extend(endpoint("other:text", 20));
            check("forward drag clamps to the originating message", selection.state().clampedToMessage && selection.state().focus.nodeId === "tail:23:text" && !selection.text().includes("This is another message."));
            selection.begin(endpoint("other:text", 12));
            selection.extend(endpoint("intro:lead", 0));
            check("backward drag clamps to the originating message", selection.state().clampedToMessage && selection.state().focus.nodeId === "other:text" && selection.state().focus.offset === 0 && selection.text() === "This is anot");
            check("starting another message clears previous highlights", intro.editor.selectedText === "" && nativeCode() === "");

            selection.begin(endpoint("code:text", 0));
            selection.selectMessage();
            check("select all stays within one message", selection.text().startsWith('Run print "hello world"') && selection.text().includes("Continuation 24.") && !selection.text().includes("This is another message."));
            check("inspector preview is bounded", selection.preview.length <= 600);
            selection.clear();
            check("clear removes native highlights", !selection.hasSelection && intro.editor.selectedText === "" && nativeCode() === "");
            host.exampleSelection();
            finish();
        }
    }
}
