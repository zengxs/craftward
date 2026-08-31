// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    property var textDocument
    property string language
    property bool darkTheme: false
    readonly property string normalizedLanguage: language.trim().toLowerCase()
    readonly property var syntaxNames: ({
            "cpp": "C++",
            "javascript": "JavaScript",
            "regex": "Regular Expression",
            "text": "Plain Text"
        })
    readonly property string syntaxName: languageRecognized ? syntaxNames[normalizedLanguage] : "Plain Text"
    readonly property bool languageRecognized: typeof syntaxNames[normalizedLanguage] === "string"
}
