// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later
// Exercise the real terminal render buffer with the application's cursor settings.

#include <contour/config/Config.hpp>
#include <vtbackend/MockTerm.hpp>

#include <iostream>

int
main(int argc, char** argv)
{
    if (argc != 2)
        return 2;
    contour::config::Config config;
    contour::config::loadConfigFromFile(config, argv[1]);
    const auto& profile = *config.profile("main");
    vtbackend::MockTerm terminalHost{ vtbackend::ColumnCount{ 20 }, vtbackend::LineCount{ 4 } };
    auto& terminal = terminalHost.terminal;
    terminal.settings().cursorMotionAnimationDuration = profile.cursorMotionAnimationDuration.value();
    terminal.setCursorShape(profile.modeInsert.value().cursor.cursorShape);
    terminal.setCursorDisplay(profile.modeInsert.value().cursor.cursorDisplay);
    const auto start = std::chrono::steady_clock::time_point{};
    terminal.tick(start + std::chrono::milliseconds{ 100 });
    terminal.ensureFreshRenderBuffer();
    terminalHost.writeToScreen("A");
    terminal.tick(start + std::chrono::milliseconds{ 200 });
    terminal.ensureFreshRenderBuffer();

    const auto buffer = terminal.renderBuffer();
    const auto& cursor = buffer.get().cursor.value();
    std::cout << "Configured cursor motion: " << profile.cursorMotionAnimationDuration.value().count()
              << " ms\nFirst updated frame: progress=" << cursor.animationProgress
              << " interpolated=" << cursor.animateFrom.has_value() << '\n';
    if (cursor.animateFrom || cursor.animationProgress < 1.0f) {
        std::cerr << "FAIL: the cursor trails its new cell after text has already advanced.\n";
        return 1;
    }
    std::cout << "PASS: the cursor reaches its new cell in the first updated render buffer.\n";
}
