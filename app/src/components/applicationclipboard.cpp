// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "applicationclipboard.h"

#include <QClipboard>
#include <QGuiApplication>

ApplicationClipboard::ApplicationClipboard(QObject* parent)
  : QObject(parent)
{
}

bool
ApplicationClipboard::copyText(const QString& text) const
{
    QClipboard* clipboard = QGuiApplication::clipboard();
    if (!clipboard)
        return false;

    clipboard->setText(text, QClipboard::Clipboard);
    return true;
}
