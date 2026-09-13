// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "terminalcontroller.h"

// An empty path selects the account's configured shell.
TerminalController::ShellCommand
terminalLoginShellCommand(const QString& shellPath = {});
