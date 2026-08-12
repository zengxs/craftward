// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "applicationiconprovider.h"
#include "ward/coreffi.h"
#include "ward/realm/realmcontroller.h"

#include <QCoreApplication>
#include <QGuiApplication>
#include <QObject>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QUrl>
#include <QVariantMap>
#include <QtQml/QQmlExtensionPlugin>

Q_IMPORT_QML_PLUGIN(Craftward_ComponentsPlugin)
Q_IMPORT_QML_PLUGIN(Craftward_EditorPlugin)
Q_IMPORT_QML_PLUGIN(Craftward_Features_LegalPlugin)
Q_IMPORT_QML_PLUGIN(Craftward_PagesPlugin)
Q_IMPORT_QML_PLUGIN(Craftward_RealmPlugin)

int
main(int argc, char* argv[])
{
    const WardCliResult cliResult = ward_core_cli_try_run(argc, argv);
    if (cliResult.handled)
        return cliResult.exit_code;

    QGuiApplication app(argc, argv);

    QCoreApplication::setApplicationName(QStringLiteral("Craftward"));
    QCoreApplication::setApplicationVersion(QStringLiteral(CRAFTWARD_VERSION));
    QCoreApplication::setOrganizationName(QStringLiteral("Craftward"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("Craftward"));

    // Use native text rendering for better font quality, especially for CJK fonts.
    QQuickWindow::setTextRenderType(QQuickWindow::NativeTextRendering);

    RealmController realmController;
    QQmlApplicationEngine engine;

    auto applicationIconProvider = createApplicationIconProvider();
    engine.addImageProvider(QStringLiteral("application-icon"), applicationIconProvider.release());

    QVariantMap initialProperties;
    initialProperties.insert(QStringLiteral("applicationIconSource"),
                             QUrl(QStringLiteral("image://application-icon/app")));
    initialProperties.insert(QStringLiteral("buildNumber"), QStringLiteral(CRAFTWARD_BUILD_NUMBER));
    initialProperties.insert(QStringLiteral("commitHash"), QStringLiteral(CRAFTWARD_COMMIT_HASH));
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
