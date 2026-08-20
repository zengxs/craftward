// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "applicationcontroller.h"
#include "applicationiconprovider.h"
#include "ward/codex/codexhistorycontroller.h"
#include "ward/coreffi.h"
#include "ward/coreffierror.h"
#include "ward/realm/realmcontroller.h"

#include <QCoreApplication>
#include <QGuiApplication>
#include <QObject>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QUrl>
#include <QVariantMap>
#include <QtQml/QQmlExtensionPlugin>

#include <memory>

Q_IMPORT_QML_PLUGIN(Craftward_ComponentsPlugin)
Q_IMPORT_QML_PLUGIN(Craftward_CodexPlugin)
Q_IMPORT_QML_PLUGIN(Craftward_EditorPlugin)
Q_IMPORT_QML_PLUGIN(Craftward_Features_LegalPlugin)
Q_IMPORT_QML_PLUGIN(Craftward_Features_RealmPlugin)
Q_IMPORT_QML_PLUGIN(Craftward_PagesPlugin)
Q_IMPORT_QML_PLUGIN(Craftward_RealmPlugin)

namespace {
struct RuntimeDeleter
{
    void operator()(WardRuntime* runtime) const { ward_core_runtime_destroy(runtime); }
};

}

int
main(int argc, char* argv[])
{
    const WardCliResult cliResult = ward_core_cli_try_run(argc, argv);
    if (cliResult.handled)
        return cliResult.exit_code;

    QGuiApplication app(argc, argv);
    app.setQuitOnLastWindowClosed(false);

    QCoreApplication::setApplicationName(QStringLiteral("Craftward"));
    QCoreApplication::setApplicationVersion(QStringLiteral(CRAFTWARD_VERSION));
    QCoreApplication::setOrganizationName(QStringLiteral("Craftward"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("Craftward"));

    // Use native text rendering for better font quality, especially for CJK fonts.
    QQuickWindow::setTextRenderType(QQuickWindow::NativeTextRendering);

    WardError* rawRuntimeError = nullptr;
    std::unique_ptr<WardRuntime, RuntimeDeleter> runtime(ward_core_runtime_create(&rawRuntimeError));
    const QString runtimeError = ward::coreffi::takeErrorMessage(rawRuntimeError);
    if (runtime == nullptr) {
        const QString message =
          runtimeError.isEmpty() ? QStringLiteral("The Ward async runtime could not be started.") : runtimeError;
        qCritical("%s", qUtf8Printable(message));
        return 1;
    }

    CodexHistoryController codexHistoryController(runtime.get());
    RealmController realmController;
    ApplicationController applicationController(app, realmController);
    QQmlApplicationEngine engine;

    auto applicationIconProvider = createApplicationIconProvider();
    engine.addImageProvider(QStringLiteral("application-icon"), applicationIconProvider.release());

    QVariantMap initialProperties;
    initialProperties.insert(QStringLiteral("applicationIconSource"),
                             QUrl(QStringLiteral("image://application-icon/app")));
    initialProperties.insert(QStringLiteral("buildNumber"), QStringLiteral(CRAFTWARD_BUILD_NUMBER));
    initialProperties.insert(QStringLiteral("commitHash"), QStringLiteral(CRAFTWARD_COMMIT_HASH));
    initialProperties.insert(QStringLiteral("applicationController"),
                             QVariant::fromValue(static_cast<QObject*>(&applicationController)));
    initialProperties.insert(QStringLiteral("codexHistoryController"),
                             QVariant::fromValue(static_cast<QObject*>(&codexHistoryController)));
    initialProperties.insert(QStringLiteral("realmController"),
                             QVariant::fromValue(static_cast<QObject*>(&realmController)));
    engine.setInitialProperties(initialProperties);

    QObject::connect(
      &engine,
      &QQmlApplicationEngine::objectCreationFailed,
      &app,
      [] { QCoreApplication::exit(-1); },
      Qt::QueuedConnection);

    engine.loadFromModule("Craftward.App", "Main");

    return app.exec();
}
