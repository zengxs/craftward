// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QHash>
#include <QObject>
#include <QVariantList>

// Experimental selection state over immutable semantic fixtures, without layout objects.
class MessageSelection : public QObject
{
    Q_OBJECT
    Q_PROPERTY(QVariantList segments READ segments WRITE setSegments NOTIFY contentChanged)
    Q_PROPERTY(QString preview READ preview NOTIFY changed)
    Q_PROPERTY(bool hasSelection READ hasSelection NOTIFY changed)

  public:
    using QObject::QObject;
    QVariantList segments() const { return segments_; }
    void setSegments(const QVariantList& segments);
    QString preview() const { return text(600); }
    bool hasSelection() const;

    Q_INVOKABLE void begin(const QVariantMap& endpoint);
    Q_INVOKABLE void extend(const QVariantMap& endpoint);
    Q_INVOKABLE void clear();
    Q_INVOKABLE void selectMessage();
    Q_INVOKABLE QVariantMap range(const QVariantList& nodes) const;
    Q_INVOKABLE QVariantMap state() const;
    Q_INVOKABLE QString text(int limit = -1) const;
    Q_INVOKABLE QString copy() const;

  signals:
    void changed();
    void contentChanged();

  private:
    struct Node
    {
        QString id;
        QString message;
        QString text;
        QString separator;
        bool control;
        int length() const { return control ? 1 : int(text.size()); }
    };
    struct Endpoint
    {
        int node = -1;
        int offset = 0;
        bool operator==(const Endpoint&) const = default;
    };
    Endpoint resolve(const QVariantMap& endpoint) const;
    QVariantMap describe(Endpoint endpoint) const;
    static bool before(Endpoint first, Endpoint second);
    QPair<Endpoint, Endpoint> ordered() const;

    QVariantList segments_;
    QList<Node> nodes_;
    QHash<QString, int> index_;
    QHash<QString, QPair<int, int>> messages_;
    Endpoint anchor_;
    Endpoint focus_;
    bool clamped_ = false;
};
