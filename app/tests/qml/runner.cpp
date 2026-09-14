// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "popuppositioner.h"

#include <QAnimationDriver>
#include <QCursor>
#include <QGuiApplication>
#include <QQmlEngine>
#include <QtQuickTest/quicktest.h>

class AnimationClock : public QAnimationDriver
{
    Q_OBJECT

  public:
    explicit AnimationClock(QObject* parent = nullptr)
      : QAnimationDriver(parent)
    {
    }

    qint64 elapsed() const override { return elapsed_; }

    Q_INVOKABLE void enable()
    {
        if (!enabled_) {
            install();
            enabled_ = true;
        }
    }

    Q_INVOKABLE void disable()
    {
        if (enabled_) {
            uninstall();
            enabled_ = false;
        }
    }

    Q_INVOKABLE bool advance(int milliseconds)
    {
        if (!enabled_ || milliseconds <= 0)
            return false;
        elapsed_ += milliseconds;
        advanceAnimation();
        return true;
    }

  protected:
    void start() override
    {
        elapsed_ = 0;
        QAnimationDriver::start();
    }

  private:
    qint64 elapsed_ = 0;
    bool enabled_ = false;
};

class NativePointer : public QObject
{
    Q_OBJECT
    Q_PROPERTY(bool available READ available CONSTANT)

  public:
    using QObject::QObject;

    bool available() const { return QGuiApplication::platformName() == "cocoa"; }
    Q_INVOKABLE QPoint position() const { return QCursor::pos(); }
    Q_INVOKABLE void moveTo(QQuickItem* item, const QPointF& position)
    {
        QCursor::setPos(item->mapToGlobal(position).toPoint());
    }
    Q_INVOKABLE void restore(const QPoint& position) { QCursor::setPos(position); }
};

class QmlTestSetup : public QObject
{
    Q_OBJECT

  public slots:
    void applicationAvailable()
    {
        qmlRegisterType<AnimationClock>("Craftward.TestSupport", 1, 0, "AnimationClock");
        qmlRegisterType<NativePointer>("Craftward.TestSupport", 1, 0, "NativePointer");
        qmlRegisterSingletonType<PopupPositioner>(
          "Craftward.Components", 1, 0, "PopupPositioner", [](QQmlEngine*, QJSEngine*) -> QObject* {
              return new PopupPositioner;
          });
    }
};

QUICK_TEST_MAIN_WITH_SETUP(craftward, QmlTestSetup)

#include "runner.moc"
