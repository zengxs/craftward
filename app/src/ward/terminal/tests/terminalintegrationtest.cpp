// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/terminal/loginshellcommand_p.h"
#include "ward/terminal/terminalcontroller.h"
#include "ward/terminal/terminalview.h"

#include <QDir>
#include <QElapsedTimer>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QKeyEvent>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickWindow>
#include <QSettings>
#include <QTemporaryDir>
#include <QTest>
#include <QTimer>
#include <contour/display/TerminalDisplay.hpp>
#include <contour/session/TerminalSession.hpp>
#include <net/Tls.hpp>
#include <vtbackend/Screen.hpp>

#include <algorithm>
#include <pwd.h>
#include <unistd.h>

Q_IMPORT_QML_PLUGIN(Craftward_TerminalPlugin)

namespace {
QByteArray
readFile(const QString& path)
{
    QFile file(path);
    return file.open(QIODevice::ReadOnly) ? file.readAll() : QByteArray();
}

TerminalController::ShellCommand
isolatedLoginShell(const QString& program, const QString& directory)
{
    auto command = terminalLoginShellCommand(program);
    // login resets HOME. Set the fixture home after login, before the unchanged bootstrap runs.
    command.arguments.insert(2, QStringLiteral("/usr/bin/env"));
    command.arguments.insert(3, QStringLiteral("HOME=") + directory);
    command.environment = { { QStringLiteral("ZDOTDIR"), directory },
                            { QStringLiteral("HISTFILE"), directory + QStringLiteral("/history") } };
    return command;
}

void
send(contour::session::TerminalSession* session, const QString& text)
{
    session->terminal().executeInput([&] { session->terminal().sendRawInput(text.toStdString()); });
}

bool
shellReady(contour::session::TerminalSession* session)
{
    const auto lock = std::scoped_lock(session->terminal());
    return session->terminal().primaryScreen().screenshot().find("test> ") != std::string::npos;
}

QString
additionalFontFamily(const TerminalController& controller)
{
    for (const auto& family : controller.fontFamilies())
        if (family != QStringLiteral("Fira Code") && family != QStringLiteral("Menlo"))
            return family;
    return {};
}
}

class TerminalIntegrationTest : public QObject
{
    Q_OBJECT

  private slots:
    void fontPreferencePersists()
    {
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const auto settingsPath = directory.filePath("settings.ini");
        QString selectedFamily;
        {
            QSettings settings(settingsPath, QSettings::IniFormat);
            TerminalController controller(settings);
            QCOMPARE(controller.fontFamily(), QStringLiteral("Fira Code"));
            QCOMPARE(controller.bundledFontFamilies(), QStringList{ QStringLiteral("Fira Code") });
            QVERIFY(controller.fontFamilies().contains(QStringLiteral("Fira Code")));
            QVERIFY(controller.fontFamilies().contains(QStringLiteral("Menlo")));
            selectedFamily = additionalFontFamily(controller);
            QVERIFY2(!selectedFamily.isEmpty(), "The test requires another installed monospaced font.");
            controller.setFontFamily(selectedFamily);
            QCOMPARE(controller.fontFamily(), selectedFamily);
            controller.setFontFamily(QStringLiteral("Craftward Missing Test Font"));
            QCOMPARE(controller.fontFamily(), selectedFamily);
        }
        QSettings restoredSettings(settingsPath, QSettings::IniFormat);
        TerminalController restored(restoredSettings);
        QCOMPARE(restored.fontFamily(), selectedFamily);
        QVERIFY(!restored.activeSession());
    }

    void unavailableFontPreference_data()
    {
        QTest::addColumn<QString>("family");
        QTest::newRow("missing") << QStringLiteral("Craftward Missing Test Font");
        QTest::newRow("empty") << QString();
    }

    void unavailableFontPreference()
    {
        QFETCH(QString, family);
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        QSettings settings(directory.filePath("settings.ini"), QSettings::IniFormat);
        settings.setValue(QStringLiteral("terminal/fontFamily"), family);
        TerminalController controller(settings);
        QCOMPARE(controller.fontFamily(), QStringLiteral("Fira Code"));
    }

    void explicitShellCommand_data()
    {
        QTest::addColumn<QString>("program");
        QTest::addColumn<QStringList>("arguments");
        QTest::addColumn<QByteArray>("expected");
        QTest::newRow("zsh") << QStringLiteral("/bin/zsh")
                             << QStringList{ "-d", "-f", "-c", "print -rn -- zsh > \"$CRAFTWARD_TEST_OUTPUT\"" }
                             << QByteArray("zsh");
        QTest::newRow("bash") << QStringLiteral("/bin/bash")
                              << QStringList{ "--noprofile",
                                              "--norc",
                                              "-c",
                                              "printf '%s' bash > \"$CRAFTWARD_TEST_OUTPUT\"" }
                              << QByteArray("bash");
    }

    void explicitShellCommand()
    {
        QFETCH(QString, program);
        QFETCH(QStringList, arguments);
        QFETCH(QByteArray, expected);
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const auto output = directory.path() + "/shell-output";
        QSettings settings(directory.path() + "/settings.ini", QSettings::IniFormat);
        TerminalController controller(settings,
                                      { program, arguments, { { QStringLiteral("CRAFTWARD_TEST_OUTPUT"), output } } });
        controller.selectConversation("shell-command", directory.path());
        controller.createTerminal();
        auto* session = dynamic_cast<contour::session::TerminalSession*>(controller.activeSession());
        QVERIFY2(session, qPrintable(controller.errorMessage()));
        session->start();
        QTRY_COMPARE(readFile(output), expected);
    }

    void builtinTlsIsUnavailable()
    {
        const auto client = net::makeTlsClientContext();
        const auto server = net::makeTlsServerContext({}, {});
        const auto selfSigned = net::makeSelfSignedServerContext();
        const auto certificate = net::generateSelfSignedCertificate();
        QVERIFY(!client.has_value());
        QVERIFY(!server.has_value());
        QVERIFY(!selfSigned.has_value());
        QVERIFY(!certificate.has_value());
        QVERIFY(QString::fromStdString(client.error()).contains("disabled"));
        QVERIFY(QString::fromStdString(server.error()).contains("disabled"));
        QVERIFY(QString::fromStdString(selfSigned.error()).contains("disabled"));
        QVERIFY(QString::fromStdString(certificate.error()).contains("disabled"));

        // Token validation remains correct when its comparison backend changes.
        QVERIFY(net::constantTimeEquals({}, {}));
        QVERIFY(net::constantTimeEquals("token", "token"));
        QVERIFY(!net::constantTimeEquals("token", "taken"));
        QVERIFY(!net::constantTimeEquals("token", "tokens"));
        QVERIFY(!net::constantTimeEquals(std::string_view("a\0b", 3), std::string_view("a\0c", 3)));
    }

    void loginShellStartup_data()
    {
        QTest::addColumn<QString>("program");
        QTest::addColumn<QString>("startupFile");
        QTest::addColumn<QByteArray>("startup");
        QTest::addColumn<bool>("unusualPath");
        const QByteArray zshStartup =
          "[[ -o login && -o interactive ]] && print -rn -- ready > \"$CRAFTWARD_TEST_READY\"\n";
        QTest::newRow("zsh") << QStringLiteral("/bin/zsh") << QStringLiteral(".zshrc") << zshStartup << false;
        QTest::newRow("bash") << QStringLiteral("/bin/bash") << QStringLiteral(".bash_profile")
                              << QByteArray("if shopt -q login_shell && [[ $- = *i* ]]; then\n"
                                            "  printf ready > \"$CRAFTWARD_TEST_READY\"\nfi\n")
                              << false;
        const QByteArray cshStartup = "if ( $?loginsh && $?prompt ) then\n"
                                      "  /usr/bin/printf ready > \"$CRAFTWARD_TEST_READY\"\nendif\n";
        QTest::newRow("tcsh") << QStringLiteral("/bin/tcsh") << QStringLiteral(".login") << cshStartup << false;
        QTest::newRow("csh") << QStringLiteral("/bin/csh") << QStringLiteral(".login") << cshStartup << false;
        QTest::newRow("shell-path-with-metacharacters")
          << QStringLiteral("/bin/zsh") << QStringLiteral(".zshrc") << zshStartup << true;
    }

    void loginShellStartup()
    {
        QFETCH(QString, program);
        QFETCH(QString, startupFile);
        QFETCH(QByteArray, startup);
        QFETCH(bool, unusualPath);
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        // Exercise physical cwd reporting even when the platform's temporary directory has no symlinks.
        const auto targetDirectory = directory.filePath(QStringLiteral("conversation target"));
        QVERIFY(QDir().mkpath(targetDirectory));
        const auto workingDirectory = directory.filePath(QStringLiteral("conversation with spaces"));
        QVERIFY(QFile::link(targetDirectory, workingDirectory));
        if (unusualPath) {
            // Keep the zsh prefix: a name starting with "sh" selects zsh's sh emulation.
            const auto linkedProgram = directory.filePath(QStringLiteral("zsh space ' $(touch should-not-run)"));
            QVERIFY(QFile::link(program, linkedProgram));
            program = linkedProgram;
        }
        QFile rc(directory.filePath(startupFile));
        QVERIFY(rc.open(QIODevice::WriteOnly));
        QCOMPARE(rc.write(startup), startup.size());
        rc.close();

        auto command = isolatedLoginShell(program, directory.path());
        command.environment.insert(QStringLiteral("CRAFTWARD_TEST_READY"), directory.filePath("ready"));
        command.environment.insert(QStringLiteral("CRAFTWARD_TEST_CWD"), directory.filePath("cwd"));
        command.environment.insert(QStringLiteral("CRAFTWARD_TEST_ENV"), directory.filePath("environment"));
        command.environment.insert(QStringLiteral("CRAFTWARD_TEST_LOGIN"), directory.filePath("login"));
        command.environment.insert(QStringLiteral("CRAFTWARD_TEST_SENTINEL"), QStringLiteral("inherited value"));
        QSettings settings(directory.filePath("settings.ini"), QSettings::IniFormat);
        TerminalController controller(settings, std::move(command));
        controller.selectConversation("login-shell", workingDirectory);
        controller.createTerminal();
        auto* session = dynamic_cast<contour::session::TerminalSession*>(controller.activeSession());
        QVERIFY2(session, qPrintable(controller.errorMessage()));
        session->start();
        QTRY_COMPARE_WITH_TIMEOUT(readFile(directory.filePath("ready")), QByteArray("ready"), 5000);

        send(session,
             "/bin/pwd -P > \"$CRAFTWARD_TEST_CWD\"\r"
             "/usr/bin/env > \"$CRAFTWARD_TEST_ENV\"\r"
             "/usr/bin/logname > \"$CRAFTWARD_TEST_LOGIN\"\r");
        QTRY_COMPARE(readFile(directory.filePath("cwd")),
                     QFileInfo(workingDirectory).canonicalFilePath().toUtf8() + '\n');
        const auto* user = getpwuid(getuid());
        QVERIFY(user && user->pw_name);
        QTRY_COMPARE(readFile(directory.filePath("login")), QByteArray(user->pw_name) + '\n');
        const auto environment = readFile(directory.filePath("environment")).split('\n');
        QVERIFY(environment.contains("CRAFTWARD_TEST_SENTINEL=inherited value"));
        QVERIFY(environment.contains("TERM=xterm-256color"));
        QVERIFY(std::none_of(environment.cbegin(), environment.cend(), [](const QByteArray& entry) {
            return entry.startsWith("STDOUT_FASTPIPE=");
        }));
        QVERIFY(!QFileInfo::exists(QDir(workingDirectory).filePath("should-not-run")));
        send(session, "exit\r");
        QTRY_COMPARE(controller.tabs()->rowCount(), 0);
    }

    void conversationProcessesAndNativeInput()
    {
        const auto originalZdotdir = qgetenv("ZDOTDIR");
        const auto originalHistory = qgetenv("HISTFILE");
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const auto firstTargetDirectory = directory.path() + "/first-target";
        const auto firstDirectory = directory.path() + "/first";
        const auto secondDirectory = directory.path() + "/second";
        QVERIFY(QDir().mkpath(firstTargetDirectory));
        QVERIFY(QFile::link(firstTargetDirectory, firstDirectory));
        QVERIFY(QDir().mkpath(secondDirectory));
        const auto expectedFirstDirectory = QFileInfo(firstDirectory).canonicalFilePath().toUtf8() + '\n';
        // Use a controlled login shell regardless of the account running the test.
        // Keep user startup files and history inside the fixture.
        QFile rc(directory.path() + "/.zshrc");
        QVERIFY(rc.open(QIODevice::WriteOnly));
        rc.write("PS1='test> '\nHISTFILE=$ZDOTDIR/history\n"
                 "[[ -o login ]] && print -rn -- zsh-login > \"$ZDOTDIR/login-shell\"\n");
        rc.close();

        QSettings settings(directory.path() + "/settings.ini", QSettings::IniFormat);
        auto shell = isolatedLoginShell(QStringLiteral("/bin/zsh"), directory.path());
        TerminalController controller(settings, std::move(shell));
        QQmlEngine engine;
        QQmlComponent component(&engine, QUrl::fromLocalFile(QString::fromUtf8(CRAFTWARD_TERMINAL_PANEL)));
        QVERIFY2(component.isReady(), qPrintable(component.errorString()));
        QQuickWindow window;
        window.resize(760, 336);
        QQmlComponent inputComponent(&engine);
        inputComponent.setData("import QtQuick.Controls\nTextField { x: 12; y: 8; width: 736; height: 36 }", QUrl());
        QVERIFY2(inputComponent.isReady(), qPrintable(inputComponent.errorString()));
        auto* hostInput = qobject_cast<QQuickItem*>(inputComponent.create());
        QVERIFY(hostInput);
        hostInput->setParent(window.contentItem());
        hostInput->setParentItem(window.contentItem());
        const auto attachPanel = [&] {
            auto* panel = qobject_cast<QQuickItem*>(component.createWithInitialProperties(
              { { QStringLiteral("controller"), QVariant::fromValue(&controller) } }));
            if (panel) {
                panel->setParent(window.contentItem());
                panel->setParentItem(window.contentItem());
                panel->setY(56);
                panel->setSize(QSizeF(760, 280));
            }
            return panel;
        };
        auto* panel = attachPanel();
        QVERIFY2(panel, qPrintable(component.errorString()));
        auto* view = panel->findChild<TerminalView*>(QStringLiteral("conversationTerminal"));
        QVERIFY(view);
        window.show();
        QVERIFY(QTest::qWaitForWindowExposed(&window));

        controller.selectConversation("first", firstDirectory);
        QVERIFY(controller.available());
        QVERIFY(!controller.panelVisible());
        controller.togglePanel();
        auto* first = dynamic_cast<contour::session::TerminalSession*>(controller.activeSession());
        QVERIFY2(first, qPrintable(controller.errorMessage()));
        QCOMPARE(controller.tabs()->rowCount(), 1);
        auto* display = view->findChild<contour::display::TerminalDisplay*>();
        QVERIFY(display);
        QTRY_VERIFY_WITH_TIMEOUT(display->hasRenderTarget(), 10000);
        QTRY_VERIFY_WITH_TIMEOUT(shellReady(first), 5000);
        QCOMPARE(readFile(directory.path() + "/login-shell"), QByteArray("zsh-login"));
        QCOMPARE(qgetenv("ZDOTDIR"), originalZdotdir);
        QCOMPARE(qgetenv("HISTFILE"), originalHistory);
        const auto firstOutput = directory.path() + "/first-cwd";
        send(first, "/bin/pwd -P > '" + firstOutput + "'\r");
        QTRY_COMPARE(readFile(firstOutput), expectedFirstDirectory);

        // A terminal click must reclaim keyboard focus from a sibling host input.
        QTRY_VERIFY(display->hasActiveFocus());
        QTest::mouseClick(&window, Qt::LeftButton, Qt::NoModifier, hostInput->mapToScene(QPointF(20, 18)).toPoint());
        QTRY_VERIFY(hostInput->hasActiveFocus());
        QTest::keyClick(&window, 'h');
        QCOMPARE(hostInput->property("text").toString(), QString("h"));
        QTest::mouseClick(&window, Qt::LeftButton, Qt::NoModifier, display->mapToScene(QPointF(20, 20)).toPoint());
        QTRY_VERIFY_WITH_TIMEOUT(display->hasActiveFocus(), 1000);
        QVERIFY(!hostInput->hasActiveFocus());
        for (const auto key : QByteArray("printf focus > focus-returned"))
            QTest::keyClick(&window, key, Qt::NoModifier, 0);
        QTest::keyClick(&window, Qt::Key_Return);
        QTRY_COMPARE(readFile(firstDirectory + "/focus-returned"), QByteArray("focus"));
        QCOMPARE(hostInput->property("text").toString(), QString("h"));

        controller.togglePanel();
        QCOMPARE(controller.activeSession(), first);
        delete panel;
        controller.togglePanel();
        panel = attachPanel();
        QVERIFY2(panel, qPrintable(component.errorString()));
        view = panel->findChild<TerminalView*>(QStringLiteral("conversationTerminal"));
        QVERIFY(view);
        display = view->findChild<contour::display::TerminalDisplay*>();
        QVERIFY(display);
        QTRY_VERIFY_WITH_TIMEOUT(display->hasRenderTarget(), 10000);
        QCOMPARE(controller.activeSession(), first);
        QCOMPARE(controller.tabs()->rowCount(), 1);
        window.hide();
        window.show();
        QCOMPARE(controller.activeSession(), first);

        // New tabs start at the conversation directory, even after another tab changes directory.
        send(first, "cd /\r");
        auto* newButton = panel->findChild<QObject*>(QStringLiteral("newTerminalButton"));
        QVERIFY(newButton);
        QVERIFY(QMetaObject::invokeMethod(newButton, "clicked"));
        auto* secondTab = dynamic_cast<contour::session::TerminalSession*>(controller.activeSession());
        QVERIFY(secondTab && secondTab != first);
        QTRY_VERIFY_WITH_TIMEOUT(shellReady(secondTab), 5000);
        const auto secondOutput = directory.path() + "/new-tab-cwd";
        send(secondTab, "/bin/pwd -P > '" + secondOutput + "'\r");
        QTRY_COMPARE(readFile(secondOutput), expectedFirstDirectory);

        controller.selectConversation("second", secondDirectory);
        QVERIFY(!controller.activeSession());
        QVERIFY(!controller.panelVisible());
        controller.togglePanel();
        auto* other = dynamic_cast<contour::session::TerminalSession*>(controller.activeSession());
        QVERIFY(other && other != secondTab);
        QTRY_VERIFY_WITH_TIMEOUT(shellReady(other), 5000);
        QCOMPARE(controller.tabs()->rowCount(), 1);
        controller.selectConversation("first", firstDirectory);
        QCOMPARE(controller.activeSession(), secondTab);
        QCOMPARE(controller.tabs()->rowCount(), 2);
        QCOMPARE(controller.activeTabIndex(), 1);

        // Font settings update existing inactive sessions as well as the visible renderer.
        const auto additionalFamily = additionalFontFamily(controller);
        QVERIFY2(!additionalFamily.isEmpty(), "The test requires another installed monospaced font.");
        controller.setFontSize(12);
        for (const auto& family : { QStringLiteral("Menlo"), additionalFamily }) {
            controller.setFontFamily(family);
            QCOMPARE(QString::fromStdString(other->profile().fonts.value().regular.familyName), family);
            QCOMPARE(QString::fromStdString(first->profile().fonts.value().regular.familyName), family);
            QTRY_COMPARE(QString::fromStdString(display->getFontDef().regular), family);
        }
        controller.setFontFamily("Fira Code");
        QTRY_COMPARE(QString::fromStdString(display->getFontDef().regular), QString("Fira Code"));

        // Physical Control-C must reach the PTY while Qt keeps Command shortcuts for the host.
        send(secondTab, "cat\r");
        QTest::qWait(150);
        QKeyEvent interrupt(QEvent::KeyPress, Qt::Key_C, Qt::MetaModifier, QString(QChar(3)));
        QCoreApplication::sendEvent(display, &interrupt);
        const auto interrupted = directory.path() + "/interrupted";
        send(secondTab, "printf done > '" + interrupted + "'\r");
        QTRY_COMPARE(readFile(interrupted), QByteArray("done"));

        // Paste 11k characters into a real Vim and verify every byte.
        send(secondTab, "/usr/bin/vim -Nu NONE -n -i NONE\r");
        const auto vimReady = [&] {
            const auto lock = std::scoped_lock(secondTab->terminal());
            return secondTab->terminal().isAlternateScreen();
        };
        QTRY_VERIFY_WITH_TIMEOUT(vimReady(), 5000);
        send(secondTab, "i");
        QTest::qWait(150);
        const QByteArray payload(11000, 'a');
        QElapsedTimer clock;
        clock.start();
        secondTab->terminal().executeInput([&] { secondTab->terminal().sendPaste(payload.toStdString()); });
        QVERIFY2(clock.elapsed() < 250, "Pasting must return promptly to the GUI event loop.");
        const auto saved = directory.path() + "/vim-paste";
        send(secondTab, "\033:call writefile(getline(1, '$'), '" + saved + "', 'b')\r");
        QTRY_COMPARE_WITH_TIMEOUT(readFile(saved), payload, 10000);

        controller.closeTab(1);
        QTRY_COMPARE(controller.tabs()->rowCount(), 1);
        QCOMPARE(controller.activeSession(), first);
        QVERIFY(controller.panelVisible());
        QVERIFY(window.isVisible());
        controller.setPanelHeight(310);
        controller.closeTab(0);
        QTRY_COMPARE(controller.tabs()->rowCount(), 0);
        QVERIFY(!controller.activeSession());
        QVERIFY(!controller.panelVisible());
        QVERIFY(window.isVisible());

        // Reopening an emptied panel starts a fresh shell and preserves the user's height.
        controller.togglePanel();
        QVERIFY(controller.panelVisible());
        QCOMPARE(controller.panelHeight(), 310);
        QCOMPARE(controller.tabs()->rowCount(), 1);
        auto* reopened = dynamic_cast<contour::session::TerminalSession*>(controller.activeSession());
        QVERIFY2(reopened, qPrintable(controller.errorMessage()));
        QTRY_VERIFY_WITH_TIMEOUT(shellReady(reopened), 5000);
        auto* firstTabs = controller.tabs();
        const auto reopenedOutput = directory.path() + "/reopened-cwd";
        send(reopened, "/bin/pwd -P > '" + reopenedOutput + "'\r");
        QTRY_COMPARE(readFile(reopenedOutput), expectedFirstDirectory);

        // A background conversation becoming empty must not hide the foreground terminal.
        controller.selectConversation("second", secondDirectory);
        QCOMPARE(controller.activeSession(), other);
        send(reopened, "exit\r");
        QTRY_COMPARE(firstTabs->rowCount(), 0);
        QVERIFY(controller.panelVisible());
        QCOMPARE(controller.activeSession(), other);
        controller.selectConversation("first", firstDirectory);
        QVERIFY(!controller.panelVisible());
        controller.selectConversation("second", secondDirectory);
        send(other, "exit\r");
        QTRY_COMPARE(controller.tabs()->rowCount(), 0);
        QVERIFY(!controller.panelVisible());
        QVERIFY(window.isVisible());

        // Destroy scene-graph views before shutting down the session service.
        delete panel;
        window.hide();
    }
};

int
main(int argc, char** argv)
{
    QQuickWindow::setGraphicsApi(QSGRendererInterface::Metal);
    QGuiApplication app(argc, argv);
    TerminalIntegrationTest test;
    return QTest::qExec(&test, argc, argv);
}

#include "terminalintegrationtest.moc"
