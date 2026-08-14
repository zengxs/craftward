// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Codex
import Craftward.Components

Rectangle {
    id: root

    required property string interactionId
    required property int kind
    required property string command
    required property string workingDirectory
    required property string reason
    required property string grantRoot
    required property var availableDecisions
    required property var questions
    required property bool blocking
    required property bool resolving
    property var answersByQuestion: ({})

    signal approvalSubmitted(int decision)
    signal userInputSubmitted(var answers)

    CodexApprovalActionState {
        id: approvalActions

        advertisedDecisions: root.availableDecisions
        defaultDecisions: [CodexHistoryController.Accept, CodexHistoryController.AcceptForSession, CodexHistoryController.Decline, CodexHistoryController.Cancel]
    }

    function collectAnswers() {
        const answers = {};
        for (const question of root.questions) {
            const questionId = question.questionId;
            answers[questionId] = Object.prototype.hasOwnProperty.call(root.answersByQuestion, questionId) ? root.answersByQuestion[questionId] : "";
        }
        return answers;
    }

    function updateAnswer(questionId, answer) {
        const updated = Object.assign({}, root.answersByQuestion);
        updated[questionId] = answer;
        root.answersByQuestion = updated;
    }

    function allQuestionsAnswered() {
        const currentAnswers = root.answersByQuestion;
        for (const question of root.questions) {
            if (!Object.prototype.hasOwnProperty.call(currentAnswers, question.questionId) || currentAnswers[question.questionId].length === 0)
                return false;
        }
        return true;
    }

    implicitHeight: contentLayout.implicitHeight + 24
    radius: 10
    color: root.palette.alternateBase
    border.color: root.palette.mid

    ColumnLayout {
        id: contentLayout

        anchors {
            fill: parent
            margins: 12
        }
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Label {
                Layout.fillWidth: true
                text: {
                    if (root.kind === CodexHistoryController.CommandApproval)
                        return qsTr("Command approval");
                    if (root.kind === CodexHistoryController.FileChangeApproval)
                        return qsTr("File change approval");
                    return qsTr("Codex needs your input");
                }
                font.weight: Font.DemiBold
            }

            BusyIndicator {
                Layout.preferredWidth: 18
                Layout.preferredHeight: 18
                running: root.resolving
                visible: running
            }

            Label {
                text: qsTr("Required")
                color: root.palette.placeholderText
                font.pixelSize: 10
                visible: root.blocking
            }
        }

        Label {
            Layout.fillWidth: true
            text: root.reason
            wrapMode: Text.WordWrap
            visible: text.length > 0
        }

        Rectangle {
            Layout.fillWidth: true
            implicitHeight: commandText.implicitHeight + 16
            radius: 6
            color: root.palette.base
            border.color: root.palette.mid
            visible: root.command.length > 0

            TextEdit {
                id: commandText

                anchors {
                    fill: parent
                    margins: 8
                }
                text: root.command
                color: root.palette.text
                font.family: Typography.monoFamily
                font.pixelSize: 11
                readOnly: true
                selectByMouse: true
                wrapMode: TextEdit.Wrap
                textFormat: TextEdit.PlainText
            }
        }

        Label {
            Layout.fillWidth: true
            text: root.workingDirectory.length > 0 ? qsTr("Working directory: %1").arg(root.workingDirectory) : qsTr("Write access: %1").arg(root.grantRoot)
            color: root.palette.placeholderText
            font.pixelSize: 10
            elide: Text.ElideMiddle
            visible: root.workingDirectory.length > 0 || root.grantRoot.length > 0
        }

        Repeater {
            id: questionRepeater

            model: root.questions

            delegate: ColumnLayout {
                id: questionEditor

                required property var modelData
                property var question: modelData

                function answer() {
                    if (otherEditor.visible && otherEditor.text.length > 0)
                        return otherEditor.text;
                    if (optionEditor.visible && optionEditor.currentIndex >= 0)
                        return questionEditor.question.options[optionEditor.currentIndex].label;
                    return textEditor.text;
                }

                function publishAnswer() {
                    root.updateAnswer(questionEditor.question.questionId, questionEditor.answer());
                }

                Component.onCompleted: publishAnswer()

                Layout.fillWidth: true
                spacing: 5

                Label {
                    Layout.fillWidth: true
                    text: questionEditor.question.header
                    font.weight: Font.DemiBold
                    visible: text.length > 0
                }

                Label {
                    Layout.fillWidth: true
                    text: questionEditor.question.prompt
                    wrapMode: Text.WordWrap
                }

                ComboBox {
                    id: optionEditor

                    Layout.fillWidth: true
                    model: questionEditor.question.options
                    textRole: "label"
                    enabled: !root.resolving
                    visible: questionEditor.question.options.length > 0
                    onCurrentIndexChanged: questionEditor.publishAnswer()
                }

                Label {
                    Layout.fillWidth: true
                    text: optionEditor.currentIndex >= 0 ? questionEditor.question.options[optionEditor.currentIndex].description : ""
                    color: root.palette.placeholderText
                    font.pixelSize: 10
                    wrapMode: Text.WordWrap
                    visible: optionEditor.visible && text.length > 0
                }

                TextField {
                    id: otherEditor

                    Layout.fillWidth: true
                    placeholderText: qsTr("Or enter another answer")
                    enabled: !root.resolving
                    echoMode: questionEditor.question.secret ? TextInput.Password : TextInput.Normal
                    visible: questionEditor.question.options.length > 0 && questionEditor.question.allowsOther
                    onTextChanged: questionEditor.publishAnswer()
                }

                TextField {
                    id: textEditor

                    Layout.fillWidth: true
                    placeholderText: qsTr("Enter your answer")
                    enabled: !root.resolving
                    echoMode: questionEditor.question.secret ? TextInput.Password : TextInput.Normal
                    visible: questionEditor.question.options.length === 0
                    onTextChanged: questionEditor.publishAnswer()
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 6
            visible: root.kind !== CodexHistoryController.UserInput

            Item {
                Layout.fillWidth: true
            }

            Button {
                text: qsTr("Cancel turn")
                enabled: !root.resolving
                visible: approvalActions.offers(CodexHistoryController.Cancel)
                onClicked: root.approvalSubmitted(CodexHistoryController.Cancel)
            }

            Button {
                text: qsTr("Decline")
                enabled: !root.resolving
                visible: approvalActions.offers(CodexHistoryController.Decline)
                onClicked: root.approvalSubmitted(CodexHistoryController.Decline)
            }

            Button {
                text: qsTr("Allow for session")
                enabled: !root.resolving
                visible: approvalActions.offers(CodexHistoryController.AcceptForSession)
                onClicked: root.approvalSubmitted(CodexHistoryController.AcceptForSession)
            }

            Button {
                text: qsTr("Allow")
                enabled: !root.resolving
                visible: approvalActions.offers(CodexHistoryController.Accept)
                highlighted: true
                onClicked: root.approvalSubmitted(CodexHistoryController.Accept)
            }
        }

        RowLayout {
            Layout.fillWidth: true
            visible: root.kind === CodexHistoryController.UserInput

            Item {
                Layout.fillWidth: true
            }

            Button {
                text: qsTr("Submit")
                enabled: !root.resolving && root.allQuestionsAnswered()
                highlighted: true
                onClicked: root.userInputSubmitted(root.collectAnswers())
            }
        }
    }
}
