// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QPointer>
#include <QQuickItem>
#include <QtQml/qqmlregistration.h>

class TerminalView : public QQuickItem
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QObject* session READ session WRITE setSession NOTIFY sessionChanged)

  public:
    explicit TerminalView(QQuickItem* parent = nullptr);
    ~TerminalView() override;
    QObject* session() const;
    void setSession(QObject* session);
    Q_INVOKABLE void focusTerminal();

  signals:
    void sessionChanged();

  protected:
    void geometryChange(const QRectF& newGeometry, const QRectF& oldGeometry) override;

  private:
    QPointer<QObject> session_;
    QQuickItem* display_;
};
