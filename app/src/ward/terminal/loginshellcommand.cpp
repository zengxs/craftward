// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "loginshellcommand_p.h"

#include <QDir>
#include <QFileInfo>

#include <pwd.h>
#include <stdexcept>
#include <unistd.h>

TerminalController::ShellCommand
terminalLoginShellCommand(const QString& shellPath)
{
    const auto* user = getpwuid(getuid());
    if (!user || !user->pw_name || !*user->pw_name)
        throw std::runtime_error("Could not determine the terminal's login account.");
    const auto userName = QString::fromLocal8Bit(user->pw_name);
    const auto home = QString::fromLocal8Bit(user->pw_dir);
    auto program = shellPath.isEmpty() ? QString::fromLocal8Bit(user->pw_shell) : shellPath;
    if (!QFileInfo(program).isExecutable())
        program = QStringLiteral("/bin/zsh");

    // login -l preserves the conversation directory but does not prefix argv[0] with '-'.
    // The bootstrap restores login-shell semantics without shell-specific command-line flags.
    // login closes extra descriptors, including Contour's optional stdout fast pipe.
    const auto flags = QFileInfo::exists(QDir(home).filePath(QStringLiteral(".hushlogin"))) ? QStringLiteral("-qflp")
                                                                                            : QStringLiteral("-flp");
    return { QStringLiteral("/usr/bin/login"),
             { flags,
               userName,
               QStringLiteral("/bin/zsh"),
               QStringLiteral("-fc"),
               QStringLiteral("unset STDOUT_FASTPIPE; exec -a \"-${1:t}\" \"$1\""),
               QStringLiteral("craftward-login"),
               program },
             {} };
}
