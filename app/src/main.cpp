#include <QCoreApplication>
#include <QGuiApplication>
#include <QObject>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QtQml/QQmlExtensionPlugin>

Q_IMPORT_QML_PLUGIN(Craftward_ComponentsPlugin)
Q_IMPORT_QML_PLUGIN(Craftward_PagesPlugin)

int
main(int argc, char* argv[])
{
    QGuiApplication app(argc, argv);

    // Use native text rendering for better font quality, especially for CJK fonts.
    QQuickWindow::setTextRenderType(QQuickWindow::NativeTextRendering);

    QQmlApplicationEngine engine;

    QObject::connect(
      &engine,
      &QQmlApplicationEngine::objectCreationFailed,
      &app,
      [] { QCoreApplication::exit(-1); },
      Qt::QueuedConnection);

    engine.loadFromModule("Craftward.App", "Main");

    return app.exec();
}
