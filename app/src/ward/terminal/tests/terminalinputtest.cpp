// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later
// Exercise the same input transaction used by the session's paste and retry paths.

#include <vtbackend/MockTerm.hpp>

#include <iostream>
#include <string>
#include <string_view>

struct InputTestTerminal : vtbackend::MockTerm<>
{
    InputTestTerminal()
      : MockTerm{ vtbackend::ColumnCount{ 40 }, vtbackend::LineCount{ 4 } }
    {
    }

    void inputPending() override { ++pendingEvents; }
    void screenUpdated() override
    {
        if (!observeUpdates)
            return;
        updateDuringInput |= insideInput;
        // The real frontend may inspect state in its callback. This accessor takes the state lock.
        (void)terminal.resolvedWindowTitle();
        ++updates;
    }

    int pendingEvents = 0;
    int updates = 0;
    bool observeUpdates = false;
    bool insideInput = false;
    bool updateDuringInput = false;
};

int
main(int argc, char** argv)
{
    const auto mode = argc == 2 ? std::string_view(argv[1]) : std::string_view("paste");
    InputTestTerminal mock;
    auto& terminal = mock.terminal;
    if (mode != "default")
        mock.writeToScreen("\033[12l");
    if (mode == "batched")
        mock.writeToScreen("\033[?2026h");

    if (mode == "parser") {
        mock.writeToScreen("\033[?996n");
        const auto replied = mock.replyData().starts_with("\033[?997;");
        mock.writeToScreen("ok");
        const auto visible = terminal.primaryScreen().grid().lineText(vtbackend::LineOffset(0)).starts_with("ok");
        std::cout << (replied && visible ? "PASS" : "FAIL") << ": parser reply echo\n";
        return replied && visible ? 0 : 1;
    }

    const auto payload = mode == "large"   ? std::string(11000, 'a')
                         : mode == "query" ? std::string("abc\033[6n")
                                           : std::string("hello");
    const auto firstLine = [&]() { return terminal.primaryScreen().grid().lineText(vtbackend::LineOffset(0)); };
    const auto blank = std::string(40, ' ');
    const auto paste = [&]() {
        terminal.executeInput([&]() {
            mock.insideInput = true;
            if (mode == "raw" || mode == "query")
                terminal.sendRawInput(payload);
            else
                terminal.sendPaste(payload);
            mock.insideInput = false;
        });
    };
    if (mode == "retry")
        mock.mockPty().setWriteBehavior(vtpty::PtyWriteBehavior::FailAgain);
    if (mode == "fatal")
        mock.mockPty().setWriteBehavior(vtpty::PtyWriteBehavior::FailFatally);
    mock.observeUpdates = true;
    std::cout << "BEGIN " << mode << std::endl;
    paste();

    bool passed = !mock.updateDuringInput;
    if (mode == "retry") {
        passed &= terminal.hasInput() && mock.pendingEvents > 0 && mock.replyData().empty() && firstLine() == blank;
        mock.mockPty().setWriteBehavior(vtpty::PtyWriteBehavior::Accept);
        terminal.executeInput([&]() { terminal.flushInput(); });
        const auto updates = mock.updates;
        terminal.executeInput([&]() { terminal.flushInput(); });
        passed &= mock.updates == updates;
    }
    if (mode == "query") {
        passed &= terminal.hasInput() && mock.pendingEvents > 0 && mock.replyData() == payload;
        terminal.executeInput([&]() { terminal.flushInput(); });
    }
    if (mode == "fatal")
        passed &= !terminal.hasInput() && mock.replyData().empty() && firstLine() == blank;
    else if (mode == "query")
        passed &= !terminal.hasInput() && mock.replyData() == payload + "\033[1;4R" &&
                  firstLine() == "abc" + std::string(37, ' ') && mock.updates > 0;
    else {
        passed &= !terminal.hasInput() && mock.replyData() == payload;
        if (mode == "default")
            passed &= firstLine() == blank && mock.updates == 0;
        else if (mode == "batched")
            passed &= firstLine() == payload + std::string(40 - payload.size(), ' ') && mock.updates == 0;
        else if (mode != "large")
            passed &= firstLine() == payload + std::string(40 - payload.size(), ' ') && mock.updates > 0;
    }
    std::cout << (passed ? "PASS: " : "FAIL: ") << mode << ", sent=" << mock.replyData().size()
              << ", updates=" << mock.updates << '\n';
    return passed ? 0 : 1;
}
