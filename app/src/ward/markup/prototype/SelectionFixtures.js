// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

.pragma library

function node(id, text, kind) {
    return {id: id, text: text, kind: kind || "text"};
}

function paragraph(id, title, text, message, kind) {
    return {id: id, message: message || "message-a", title: title, kind: kind || "prose",
        columns: 1, parts: [{id: id, nodes: [node(id + ":text", text, kind === "code" ? "code" : "text")]}]};
}

function segments() {
    const result = [
        {id: "intro", message: "message-a", title: "MESSAGE A / MIXED INLINE TEXT", kind: "prose", columns: 1,
            parts: [{id: "intro", nodes: [node("intro:lead", "Run "), node("intro:code", 'print "hello world"', "code"),
                node("intro:tail", ". Drag from this paragraph into the code and table below. Unicode: 你好 · café · 👩‍💻 · مرحبا بالعالم.")]}]},
        paragraph("paragraphs", "TWO PARAGRAPHS / ONE SEGMENT", "One segment can contain more than one paragraph.\n\nThe selection follows the message's text order across rendering fragments."),
        paragraph("code", "CODE / WHITESPACE IS PRESERVED", 'def greet(name):\n    print("hello " + name)\n\n# Keep the indentation and blank line.\ngreet("world")', "message-a", "code"),
        {id: "table", message: "message-a", title: "TABLE / INLINE NODES AND ROW ORDER", kind: "table", columns: 3,
            parts: ["Name", "State", "Note", "Alpha ", "Ready", "First row ", "Beta", "Waiting", "第二行 👋"].map((text, i) => {
                const nodes = [node("cell:" + i + ":text", text)];
                if (i === 3)
                    nodes.push(node("cell:3:code", "a()", "code"));
                if (i === 4) {
                    nodes[0].kind = "link";
                    nodes[0].target = "prototype:state/ready";
                    nodes[0].hint = "This item is ready for review.";
                }
                if (i === 5) {
                    nodes.push({id: "cell:5:annotation", kind: "annotation", text: "[4]",
                        target: "codex-annotation:4", hint: "Annotation 4: review this statement.", source: ':codex-annotation{index="4"}'});
                }
                return {id: "cell:" + i, nodes: nodes};
            })},
        paragraph("after", "AFTER THE TABLE", "The same selection continues after the table. Scroll away and return: the highlight should survive the text items being destroyed and recreated.")
    ];
    for (let i = 0; i < 24; ++i)
        result.push(paragraph("tail:" + i, "MESSAGE A / CONTINUATION " + (i + 1),
            "Continuation " + (i + 1) + ". This text belongs to the same message. Only nearby segments need text documents; the logical selection and clipboard content do not depend on which segments are on screen."));
    result.push(paragraph("other", "MESSAGE B / A SEPARATE SELECTION SCOPE", "This is another message. A drag that starts above stops at the end of Message A. Start a new selection here to select Message B.", "message-b"));
    for (let i = 0; i < 12; ++i)
        result.push(paragraph("other:" + i, "MESSAGE B / CONTINUATION " + (i + 1), "Another message, another selection scope. These extra segments let you observe the bounded set of live text documents while scrolling.", "message-b"));
    return result;
}
