// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QHash>
#include <QIdentityProxyModel>
#include <QMetaObject>
#include <QVector>
#include <QtQmlIntegration/qqmlintegration.h>

class CodexTimelinePageModel : public QIdentityProxyModel
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(int turnsPerPage READ turnsPerPage WRITE setTurnsPerPage NOTIFY turnsPerPageChanged)
    Q_PROPERTY(int totalRowCount READ totalRowCount NOTIFY statisticsChanged)
    Q_PROPERTY(int pageCount READ pageCount NOTIFY statisticsChanged)
    Q_PROPERTY(int revision READ revision NOTIFY revisionChanged)

  public:
    enum Role
    {
        DetailRowRole = Qt::UserRole + 0x1000,
        FirstDetailInTurnRole,
        DetailCountInTurnRole,
    };

    explicit CodexTimelinePageModel(QObject* parent = nullptr);

    void setSourceModel(QAbstractItemModel* model) override;

    [[nodiscard]] int turnsPerPage() const;
    void setTurnsPerPage(int turns);

    [[nodiscard]] int totalRowCount() const;
    [[nodiscard]] int pageCount() const;
    [[nodiscard]] int revision() const;

    [[nodiscard]] QVariant data(const QModelIndex& index, int role) const override;
    [[nodiscard]] QHash<int, QByteArray> roleNames() const override;

    Q_INVOKABLE [[nodiscard]] QVariant valueAt(int row, const QString& roleName) const;
    Q_INVOKABLE [[nodiscard]] int pageFirstRow(int page) const;
    Q_INVOKABLE [[nodiscard]] int pageRowCount(int page) const;
    Q_INVOKABLE [[nodiscard]] QString pageId(int page) const;

  signals:
    void turnsPerPageChanged();
    void statisticsChanged();
    void revisionChanged();

  private:
    struct RowMetadata
    {
        QString entryId;
        bool detail = false;
        bool firstDetailInTurn = false;
        int detailCountInTurn = 0;
    };

    struct Page
    {
        int firstRow = 0;
        int rowCount = 0;
        QString id;
    };

    [[nodiscard]] QVariant sourceValue(int sourceRow, const QByteArray& roleName) const;
    [[nodiscard]] int roleForName(const QByteArray& roleName) const;
    void reconnectRoles();
    void recomputeSnapshot();
    void rebuildPages();
    void connectSourceSignals(QAbstractItemModel* model);
    void disconnectSourceSignals();
    void refreshFromSource();
    void advanceRevision();

    QHash<QByteArray, int> rolesByName_;
    QVector<QMetaObject::Connection> sourceConnections_;
    QVector<RowMetadata> metadata_;
    QVector<Page> pages_;
    int turnsPerPage_ = 8;
    int revision_ = 0;
};
