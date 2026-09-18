// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QObject>
#include <QPointer>
#include <QQuickItem>
#include <QQuickWindow>
#include <QtQml/qqmlregistration.h>

class PopupAnchorToggle : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QQuickItem* anchorItem READ anchorItem WRITE setAnchorItem NOTIFY anchorItemChanged)
    Q_PROPERTY(QQuickWindow* popupWindow READ popupWindow WRITE setPopupWindow NOTIFY popupWindowChanged)
    Q_PROPERTY(bool enabled READ enabled WRITE setEnabled NOTIFY enabledChanged)

  public:
    using QObject::QObject;

    QQuickItem* anchorItem() const { return anchorItem_; }
    void setAnchorItem(QQuickItem* item);
    QQuickWindow* popupWindow() const { return popupWindow_; }
    void setPopupWindow(QQuickWindow* window);
    bool enabled() const { return enabled_; }
    void setEnabled(bool enabled);

  signals:
    void anchorItemChanged();
    void popupWindowChanged();
    void enabledChanged();
    void activated();

  protected:
    bool eventFilter(QObject* watched, QEvent* event) override;

  private:
    QPointer<QQuickItem> anchorItem_;
    QPointer<QQuickWindow> popupWindow_;
    bool enabled_ = false;
};
