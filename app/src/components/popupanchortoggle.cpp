// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "popupanchortoggle.h"

#include <QMouseEvent>

void
PopupAnchorToggle::setAnchorItem(QQuickItem* item)
{
    if (anchorItem_ == item)
        return;
    anchorItem_ = item;
    emit anchorItemChanged();
}

void
PopupAnchorToggle::setPopupWindow(QQuickWindow* window)
{
    if (popupWindow_ == window)
        return;
    if (popupWindow_)
        popupWindow_->removeEventFilter(this);
    popupWindow_ = window;
    if (popupWindow_)
        popupWindow_->installEventFilter(this);
    emit popupWindowChanged();
}

void
PopupAnchorToggle::setEnabled(bool enabled)
{
    if (enabled_ == enabled)
        return;
    enabled_ = enabled;
    emit enabledChanged();
}

bool
PopupAnchorToggle::eventFilter(QObject* watched, QEvent* event)
{
    if (!enabled_ || watched != popupWindow_ || !anchorItem_ || !anchorItem_->isVisible() || !anchorItem_->isEnabled())
        return false;
    if (event->type() != QEvent::MouseButtonPress && event->type() != QEvent::MouseButtonDblClick)
        return false;
    const auto* mouse = static_cast<QMouseEvent*>(event);
    if (mouse->button() != Qt::LeftButton ||
        !anchorItem_->contains(anchorItem_->mapFromGlobal(mouse->globalPosition())))
        return false;

    // Consume the native press before Qt closes the popup and replays it to its anchor.
    event->accept();
    emit activated();
    return true;
}
