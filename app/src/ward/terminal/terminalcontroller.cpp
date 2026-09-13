// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "terminalcontroller.h"
#include "loginshellcommand_p.h"

#include <contour/ContourGuiApp.hpp>
#include <contour/config/Config.hpp>
#include <contour/display/TerminalAccessible.hpp>
#include <contour/session/TerminalSession.hpp>
#include <contour/session/TerminalSessionManager.hpp>
#include <contour/window/WindowController.hpp>
#include <crispy/Environment.hpp>

#include <CoreText/CoreText.h>
#include <QCoreApplication>
#include <QDir>
#include <QEvent>
#include <QFileInfo>
#include <QFontDatabase>
#include <QGuiApplication>
#include <QHash>
#include <QPointer>
#include <QSettings>

#include <algorithm>
#include <optional>

namespace {
using contour::window::WindowController;

QString
resourcePath(const QString& relative, const QString& development)
{
    const auto bundled = QCoreApplication::applicationDirPath() + QStringLiteral("/../Resources/") + relative;
    return QFileInfo::exists(bundled) ? bundled : development;
}

QStringList
registerTerminalFonts()
{
    static bool initialized = false;
    static QStringList families;
    if (initialized)
        return families;
    const auto directory =
      resourcePath(QStringLiteral("fonts/firacode"), QString::fromUtf8(CRAFTWARD_TERMINAL_FONT_DIRECTORY));
    for (const auto* name : { "FiraCode-Regular.ttf", "FiraCode-Bold.ttf" }) {
        const auto path = QDir(directory).filePath(QString::fromLatin1(name));
        const auto bytes = QFile::encodeName(path);
        const auto url = CFURLCreateFromFileSystemRepresentation(
          kCFAllocatorDefault, reinterpret_cast<const UInt8*>(bytes.constData()), bytes.size(), false);
        if (!url)
            continue;
        CFErrorRef error = nullptr;
        const bool registered = CTFontManagerRegisterFontsForURL(url, kCTFontManagerScopeProcess, &error);
        if (!registered && error && CFErrorGetCode(error) != kCTFontManagerErrorAlreadyRegistered)
            qWarning() << "Could not register terminal font:" << path;
        if (error)
            CFRelease(error);
        CFRelease(url);
        const auto id = QFontDatabase::addApplicationFont(path);
        if (id >= 0)
            families.append(QFontDatabase::applicationFontFamilies(id));
    }
    families.removeDuplicates();
    initialized = true;
    return families;
}

QStringList
terminalFontFamilies()
{
    const auto bundledFamilies = registerTerminalFonts();
    auto families = QFontDatabase::families();
    families.removeIf([](const QString& family) {
        return QFontDatabase::isPrivateFamily(family) || !QFontDatabase::isFixedPitch(family);
    });
    families.sort(Qt::CaseInsensitive);
    for (auto it = bundledFamilies.crbegin(); it != bundledFamilies.crend(); ++it)
        if (families.removeAll(*it))
            families.prepend(*it);
    return families;
}
}

struct TerminalController::Private
{
    struct Conversation
    {
        QPointer<WindowController> tabs;
        bool visible = false;
    };

    explicit Private(QSettings& value)
      : settings(value)
    {
        families = terminalFontFamilies();
        family = settings.value(QStringLiteral("terminal/fontFamily"), QStringLiteral("Fira Code")).toString();
        if (!families.contains(family))
            family = families.value(0);
        size = std::clamp(settings.value(QStringLiteral("terminal/fontSize"), 10).toInt(), 8, 32);
        height = std::clamp(settings.value(QStringLiteral("terminal/panelHeight"), 240).toInt(), 120, 1200);
    }

    WindowController* current() const { return conversations.value(threadId).tabs; }

    void initialize()
    {
        if (backend)
            return;
        auto candidate = std::make_unique<contour::ContourGuiApp>(environment);
        const char* arguments[] = { "contour", "terminal" };
        if (!candidate->reparseParameters(2, arguments))
            throw std::runtime_error("Could not initialize the embedded terminal.");
        contour::config::loadConfigFromFile(
          candidate->config(),
          resourcePath(QStringLiteral("terminal/contour.yml"), QString::fromUtf8(CRAFTWARD_TERMINAL_CONFIG))
            .toStdString());
        auto& shell = candidate->config().profile("main")->shell.value();
        const auto command = shellCommand ? *shellCommand : terminalLoginShellCommand();
        shell.program = command.program.toStdString();
        shell.arguments.clear();
        for (const auto& argument : command.arguments)
            shell.arguments.push_back(argument.toStdString());
        for (auto it = command.environment.cbegin(); it != command.environment.cend(); ++it)
            shell.env[it.key().toStdString()] = it.value().toStdString();
        contour::display::TerminalAccessible::installFactory();
        backend = std::move(candidate);
        applyFonts();
    }

    void applyFonts()
    {
        if (!backend)
            return;
        auto fonts = backend->config().profile("main")->fonts.value();
        fonts.size = text::FontSize{ static_cast<double>(size) };
        for (auto* face : { &fonts.regular, &fonts.bold, &fonts.italic, &fonts.boldItalic })
            face->familyName = family.toStdString();
        backend->config().profile("main")->fonts.value() = fonts;
        auto& manager = backend->sessionsManager();
        for (const auto& conversation : conversations) {
            if (!conversation.tabs)
                continue;
            auto* window = manager.model().window(conversation.tabs->windowId());
            if (!window)
                continue;
            for (int row = 0; row < window->tabCount(); ++row)
                for (auto* session : manager.sessionsOfTab(window->tabAt(row)))
                    session->setFonts(fonts);
        }
    }

    QSettings& settings;
    std::optional<ShellCommand> shellCommand;
    crispy::LiveEnvironment environment;
    std::unique_ptr<contour::ContourGuiApp> backend;
    QHash<QString, Conversation> conversations;
    QString threadId;
    QString directory;
    QString error;
    QStringList families;
    QString family;
    int size = 10;
    int height = 240;
};

TerminalController::TerminalController(QSettings& settings, QObject* parent)
  : QObject(parent)
  , d(std::make_unique<Private>(settings))
{
    connect(qGuiApp, &QGuiApplication::fontDatabaseChanged, this, [this] {
        const auto families = terminalFontFamilies();
        if (families == d->families)
            return;
        d->families = families;
        emit fontFamiliesChanged();
        if (!families.contains(d->family))
            setFontFamily(families.value(0));
    });
}

TerminalController::TerminalController(QSettings& settings, ShellCommand shell, QObject* parent)
  : TerminalController(settings, parent)
{
    d->shellCommand = std::move(shell);
}

TerminalController::~TerminalController()
{
    for (const auto& conversation : d->conversations)
        if (conversation.tabs)
            conversation.tabs->closeWindow();
    // Contour schedules session/controller destruction; finish it while its backend is alive.
    QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
}

void
TerminalController::selectConversation(const QString& threadId, const QString& workingDirectory)
{
    if (d->threadId == threadId && d->directory == workingDirectory)
        return;
    d->threadId = threadId;
    d->directory = workingDirectory;
    d->error.clear();
    emit conversationChanged();
    emit viewChanged();
}

bool
TerminalController::available() const
{
    return !d->threadId.isEmpty() && !d->directory.isEmpty() && QFileInfo(d->directory).isDir();
}
bool
TerminalController::panelVisible() const
{
    return d->conversations.value(d->threadId).visible;
}
QAbstractItemModel*
TerminalController::tabs() const
{
    return d->current();
}
QObject*
TerminalController::activeSession() const
{
    return d->current() ? d->current()->activeSession() : nullptr;
}
int
TerminalController::activeTabIndex() const
{
    return d->current() ? d->current()->activeTabIndex() : -1;
}
QString
TerminalController::errorMessage() const
{
    return d->error;
}
QStringList
TerminalController::fontFamilies() const
{
    return d->families;
}
QStringList
TerminalController::bundledFontFamilies() const
{
    return registerTerminalFonts();
}
QString
TerminalController::fontFamily() const
{
    return d->family;
}
int
TerminalController::fontSize() const
{
    return d->size;
}
int
TerminalController::panelHeight() const
{
    return d->height;
}

void
TerminalController::setFontFamily(const QString& family)
{
    if (d->family == family || !d->families.contains(family))
        return;
    d->family = family;
    d->settings.setValue(QStringLiteral("terminal/fontFamily"), family);
    d->applyFonts();
    emit fontChanged();
}

void
TerminalController::setFontSize(int size)
{
    size = std::clamp(size, 8, 32);
    if (d->size == size)
        return;
    d->size = size;
    d->settings.setValue(QStringLiteral("terminal/fontSize"), size);
    d->applyFonts();
    emit fontChanged();
}

void
TerminalController::setPanelHeight(int height)
{
    height = std::clamp(height, 120, 1200);
    if (d->height == height)
        return;
    d->height = height;
    d->settings.setValue(QStringLiteral("terminal/panelHeight"), height);
    emit panelHeightChanged();
}

void
TerminalController::togglePanel()
{
    if (!available() && !d->current())
        return;
    auto& conversation = d->conversations[d->threadId];
    conversation.visible = !conversation.visible;
    if (conversation.visible && (!conversation.tabs || conversation.tabs->count() == 0))
        createTerminal();
    emit viewChanged();
    if (conversation.visible)
        emit focusRequested();
}

void
TerminalController::createTerminal()
{
    if (!available())
        return;
    try {
        d->initialize();
        auto& conversation = d->conversations[d->threadId];
        if (!conversation.tabs) {
            conversation.tabs =
              d->backend->sessionsManager().createWindowController(contour::window::WindowMode::Embedded);
            connect(conversation.tabs, &WindowController::activeSessionChanged, this, &TerminalController::viewChanged);
            connect(
              conversation.tabs, &WindowController::activeTabIndexChanged, this, &TerminalController::viewChanged);
            connect(conversation.tabs, &WindowController::countChanged, this, [this, threadId = d->threadId] {
                auto& conversation = d->conversations[threadId];
                if (conversation.tabs && conversation.tabs->count() == 0)
                    conversation.visible = false;
                if (d->threadId == threadId)
                    emit viewChanged();
            });
        }
        auto* session = d->backend->sessionsManager().createSession(
          conversation.tabs->windowId(), std::nullopt, d->directory.toStdString());
        if (!session)
            throw std::runtime_error("The terminal process could not be created.");
        conversation.tabs->activateTab(conversation.tabs->count() - 1);
        conversation.visible = true;
        d->error.clear();
    } catch (const std::exception& error) {
        d->error = QString::fromUtf8(error.what());
        d->conversations[d->threadId].visible = true;
    }
    emit viewChanged();
    emit focusRequested();
}

void
TerminalController::activateTab(int index)
{
    if (auto* tabs = d->current())
        tabs->activateTab(index);
    emit focusRequested();
}

void
TerminalController::closeTab(int index)
{
    if (auto* tabs = d->current())
        tabs->closeTabAtIndex(index);
    emit focusRequested();
}
