// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

// Native capability prototype, with fixture-built semantics and no persistence.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: window
    width: 1120
    height: 820
    minimumWidth: 900
    minimumHeight: 680
    visible: true
    title: "Craftward Inline Prototype"
    property bool dark: false
    property string lastInteraction: "Select text across nodes, hover a link, or click the inline control."
    property int appended: 0
    readonly property color muted: dark ? "#adb5c1" : "#657080"
    color: dark ? "#181b21" : "#f4f6f9"
    palette.window: color
    palette.windowText: dark ? "#e7e9ee" : "#20252e"
    palette.text: dark ? "#e7e9ee" : "#20252e"
    palette.base: dark ? "#232730" : "white"
    palette.button: dark ? "#303743" : "#e6ebf3"
    palette.buttonText: dark ? "#e7e9ee" : "#20252e"
    palette.highlight: "#426da5"
    palette.highlightedText: "white"

    function snapshot() {
        return {
            dark,
            fontSize: fontSize.value,
            lastInteraction,
            blocks: [intro.snapshot(), controls.snapshot(), streaming.snapshot()]
        };
    }

    Component.onCompleted: {
        if (probeExpanded)
            Qt.callLater(() => controls.bridge.toggleControl("control:review"));
    }

    Connections {
        target: window
        enabled: probeMode
        property bool reported: false
        function onFrameSwapped() {
            if (reported)
                return;
            reported = true;
            Qt.callLater(() => prototypeCapture.report(window.snapshot()));
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 24
        spacing: 16
        RowLayout {
            Layout.fillWidth: true
            ColumnLayout {
                Label {
                    text: "Inline renderer / native prototype"
                    font.pixelSize: 23
                    font.bold: true
                }
                Label {
                    text: "Semantic nodes, native text selection, and an interactive inline control"
                    color: window.muted
                }
            }
            Item {
                Layout.fillWidth: true
            }
            Switch {
                text: "Dark"
                checked: window.dark
                onToggled: window.dark = checked
            }
        }

        RowLayout {
            Label {
                text: "Text width"
            }
            Slider {
                id: textWidth
                from: 280
                to: 690
                value: probeWidth || 610
                stepSize: 1
                Layout.preferredWidth: 190
            }
            Label {
                text: Math.round(textWidth.value) + " px"
                Layout.preferredWidth: 55
            }
            Label {
                text: "Font"
            }
            Slider {
                id: fontSize
                from: 14
                to: 24
                value: probeFontSize || 17
                stepSize: 1
                Layout.preferredWidth: 100
            }
            Label {
                text: Math.round(fontSize.value) + " px"
            }
            Item {
                Layout.fillWidth: true
            }
            Button {
                text: "Capture state"
                onClicked: prototypeCapture.save(window, window.snapshot())
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 20
            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                contentWidth: availableWidth
                clip: true
                Column {
                    id: content
                    width: parent.width
                    spacing: 24
                    Label {
                        text: "1 / TEXT, CODE, LINKS, ANNOTATIONS"
                        color: window.muted
                        font.pixelSize: 11
                        font.bold: true
                    }
                    InlineBlock {
                        id: intro
                        blockId: "intro"
                        width: Math.min(textWidth.value, content.width)
                        height: implicitHeight
                        dark: window.dark
                        pixelSize: fontSize.value
                        onInteraction: description => window.lastInteraction = description
                        nodes: [
                            {
                                id: "intro:plain",
                                kind: "text",
                                text: "Run "
                            },
                            {
                                id: "intro:code",
                                kind: "code",
                                text: 'print "hello world"'
                            },
                            {
                                id: "intro:bridge",
                                kind: "text",
                                text: " and read the "
                            },
                            {
                                id: "intro:link",
                                kind: "link",
                                text: "Qt text document guide with a deliberately long link label",
                                target: "https://doc.qt.io/qt-6/qtextdocument.html"
                            },
                            {
                                id: "intro:after-link",
                                kind: "text",
                                text: ". A custom annotation appears inline "
                            },
                            {
                                id: "intro:annotation",
                                kind: "annotation",
                                text: "[4]",
                                target: "codex-annotation:4",
                                source: ':codex-annotation{index="4"}'
                            },
                            {
                                id: "intro:tail",
                                kind: "text",
                                text: ". Select across all of these nodes. Unicode sample: 你好，世界 · café · 👩‍💻 · مرحبا بالعالم."
                            }
                        ]
                    }
                    Rectangle {
                        width: content.width
                        height: 1
                        color: window.dark ? "#363d48" : "#dce1e9"
                    }
                    Label {
                        text: "2 / A REAL QML CONTROL IN THE TEXT FLOW"
                        color: window.muted
                        font.pixelSize: 11
                        font.bold: true
                    }
                    InlineBlock {
                        id: controls
                        blockId: "controls"
                        width: Math.min(textWidth.value, content.width)
                        height: implicitHeight
                        dark: window.dark
                        pixelSize: fontSize.value
                        onInteraction: description => window.lastInteraction = description
                        nodes: [
                            {
                                id: "control:before",
                                kind: "text",
                                text: "This paragraph owns a real inline button "
                            },
                            {
                                id: "control:review",
                                kind: "control",
                                text: "Review",
                                expanded: false
                            },
                            {
                                id: "control:after",
                                kind: "text",
                                text: " whose label and reserved width change when clicked. The surrounding text wraps around it; the paragraphs above and below keep their document instances."
                            }
                        ]
                    }
                    Rectangle {
                        width: content.width
                        height: 1
                        color: window.dark ? "#363d48" : "#dce1e9"
                    }
                    Label {
                        text: "3 / STREAMING A STABLE TAIL NODE"
                        color: window.muted
                        font.pixelSize: 11
                        font.bold: true
                    }
                    InlineBlock {
                        id: streaming
                        blockId: "streaming"
                        width: Math.min(textWidth.value, content.width)
                        height: implicitHeight
                        dark: window.dark
                        pixelSize: fontSize.value
                        onInteraction: description => window.lastInteraction = description
                        nodes: [
                            {
                                id: "stream:lead",
                                kind: "text",
                                text: "Inside a code span, the directive stays literal: "
                            },
                            {
                                id: "stream:code",
                                kind: "code",
                                text: ':codex-annotation{index="4"}'
                            },
                            {
                                id: "stream:tail",
                                kind: "text",
                                text: ". New text is appended to this same tail node."
                            }
                        ]
                    }
                    Row {
                        spacing: 8
                        Button {
                            text: "Append text"
                            onClicked: {
                                streaming.bridge.appendText(" Chunk " + (++window.appended) + ": hello 世界 👋.");
                                window.lastInteraction = "Appended to stream:tail";
                            }
                        }
                        Button {
                            text: "Select mixed text"
                            onClicked: {
                                intro.editor.forceActiveFocus();
                                intro.editor.selectAll();
                            }
                        }
                        Button {
                            text: "Select control paragraph"
                            onClicked: {
                                controls.editor.forceActiveFocus();
                                controls.editor.selectAll();
                            }
                        }
                    }
                }
            }

            Rectangle {
                Layout.fillHeight: true
                width: 1
                color: window.dark ? "#363d48" : "#dce1e9"
            }
            ScrollView {
                Layout.preferredWidth: 280
                Layout.fillHeight: true
                contentWidth: availableWidth
                Column {
                    width: parent.width
                    spacing: 16
                    Label {
                        text: "EXPERIMENT STATE"
                        font.bold: true
                        font.pixelSize: 11
                        color: window.muted
                    }
                    Label {
                        text: window.lastInteraction
                        width: parent.width
                        wrapMode: Text.Wrap
                    }
                    Label {
                        text: "Selection as plain text"
                        font.bold: true
                    }
                    Label {
                        width: parent.width
                        text: controls.semanticSelection || intro.semanticSelection || streaming.semanticSelection || "No selection"
                        wrapMode: Text.Wrap
                    }
                    Label {
                        text: "Document and node state"
                        font.bold: true
                    }
                    TextArea {
                        width: parent.width
                        readOnly: true
                        selectByMouse: true
                        wrapMode: TextEdit.Wrap
                        font.pixelSize: 11
                        text: {
                            intro.bridge.revision;
                            controls.bridge.revision;
                            streaming.bridge.revision;
                            controls.layoutRevision;
                            return JSON.stringify(window.snapshot(), null, 2);
                        }
                    }
                }
            }
        }
        Label {
            text: "Throwaway capability experiment · fixture-built nodes · no production timeline integration"
            color: window.muted
            font.pixelSize: 11
        }
    }
}
