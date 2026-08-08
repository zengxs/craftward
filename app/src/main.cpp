#include "applicationiconprovider.h"

#include <QCoreApplication>
#include <QGuiApplication>
#include <QObject>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QUrl>
#include <QVariantMap>
#include <QtQml/QQmlExtensionPlugin>

Q_IMPORT_QML_PLUGIN(Craftward_ComponentsPlugin)
Q_IMPORT_QML_PLUGIN(Craftward_Features_LegalPlugin)
Q_IMPORT_QML_PLUGIN(Craftward_PagesPlugin)

int
main(int argc, char* argv[])
{
    QGuiApplication app(argc, argv);

    QCoreApplication::setApplicationName(QStringLiteral("Craftward"));
    QCoreApplication::setApplicationVersion(QStringLiteral(CRAFTWARD_VERSION));
    QCoreApplication::setOrganizationName(QStringLiteral("Craftward"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("Craftward"));

    // Use native text rendering for better font quality, especially for CJK fonts.
    QQuickWindow::setTextRenderType(QQuickWindow::NativeTextRendering);

    QQmlApplicationEngine engine;

    auto applicationIconProvider = createApplicationIconProvider();
    engine.addImageProvider(QStringLiteral("application-icon"), applicationIconProvider.release());

    QVariantMap initialProperties;
    initialProperties.insert(QStringLiteral("applicationIconSource"),
                             QUrl(QStringLiteral("image://application-icon/app")));
    initialProperties.insert(QStringLiteral("buildNumber"), QStringLiteral(CRAFTWARD_BUILD_NUMBER));
    initialProperties.insert(QStringLiteral("commitHash"), QStringLiteral(CRAFTWARD_COMMIT_HASH));
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
