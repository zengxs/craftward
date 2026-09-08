// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "markuprenderplan.h"

#include <QObject>
#include <QTextBoundaryFinder>
#include <QtQmlIntegration/qqmlintegration.h>

/// Logical selection for one message, independent of materialized text documents.
class MarkupSelection : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Selection is owned by a markup document.")
    Q_PROPERTY(bool hasSelection READ hasSelection NOTIFY changed)

  public:
    explicit MarkupSelection(QObject* parent = nullptr);
    void reconcile(const QList<MarkupTextSurface>& surfaces);
    [[nodiscard]] bool hasSelection() const;
    Q_INVOKABLE void begin(const QVariantMap& endpoint);
    Q_INVOKABLE void extend(const QVariantMap& endpoint);
    Q_INVOKABLE void clear();
    Q_INVOKABLE void selectAll();
    Q_INVOKABLE [[nodiscard]] QVariantMap range(const QVariant& surface) const;
    Q_INVOKABLE [[nodiscard]] QString text() const;
    Q_INVOKABLE void copy() const;

  signals:
    void changed();

  private:
    struct Run
    {
        QString key;
        int start = 0;
        int length = 0;
    };
    [[nodiscard]] int position(const QVariantMap& endpoint) const;
    [[nodiscard]] QVariantMap endpoint(int position) const;
    [[nodiscard]] QVariantMap normalize(const QVariantMap& endpoint) const;

    QList<Run> runs_;
    QHash<QString, int> indices_;
    QString text_;
    mutable QTextBoundaryFinder boundaries_;
    QVariantMap anchor_;
    QVariantMap focus_;
};
