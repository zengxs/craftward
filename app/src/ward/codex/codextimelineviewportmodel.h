// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QAbstractListModel>
#include <QHash>
#include <QMetaObject>
#include <QPointer>
#include <QString>
#include <QVector>
#include <QtQmlIntegration/qqmlintegration.h>

class CodexTimelineViewportModel : public QAbstractListModel
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QAbstractItemModel* sourceModel READ sourceModel WRITE setSourceModel NOTIFY sourceModelChanged)
    Q_PROPERTY(int totalRowCount READ totalRowCount NOTIFY statisticsChanged)
    Q_PROPERTY(int revision READ revision NOTIFY revisionChanged)

  public:
    enum Role
    {
        SourceEntryIdRole = Qt::UserRole + 0x4000,
        SemanticBlockRole,
        BlockIdRole,
        BlockIndexRole,
        BlockCountRole,
        CodeBlockRole,
        BlockTextRole,
        PlainTextRole,
        LanguageRole,
        MarkdownRole,
        FirstBlockInEntryRole,
        LastBlockInEntryRole,
    };
    Q_ENUM(Role)

    explicit CodexTimelineViewportModel(QObject* parent = nullptr);

    [[nodiscard]] QAbstractItemModel* sourceModel() const;
    void setSourceModel(QAbstractItemModel* model);

    [[nodiscard]] int rowCount(const QModelIndex& parent = {}) const override;
    [[nodiscard]] int totalRowCount() const;
    [[nodiscard]] int revision() const;
    [[nodiscard]] QVariant data(const QModelIndex& index, int role) const override;
    [[nodiscard]] QHash<int, QByteArray> roleNames() const override;

    Q_INVOKABLE [[nodiscard]] QVariant valueAt(int row, const QString& roleName) const;
    Q_INVOKABLE [[nodiscard]] QString entryIdAt(int row) const;
    Q_INVOKABLE [[nodiscard]] int indexOfEntryId(const QString& entryId) const;

  signals:
    void sourceModelChanged();
    void statisticsChanged();
    void revisionChanged();

  private:
    struct ViewportRow
    {
        int sourceRow = -1;
        QPointer<QAbstractItemModel> blockModel;
        int blockRow = -1;
        QString entryId;
        QString sourceEntryId;
    };

    struct BlockModelSubscription
    {
        QList<int> sourceRows;
        QVector<QMetaObject::Connection> connections;
    };

    [[nodiscard]] int roleForName(const QByteArray& roleName) const;
    [[nodiscard]] QVariant sourceValue(const ViewportRow& row, int role) const;
    [[nodiscard]] QVariant blockValue(const ViewportRow& row, const QByteArray& roleName) const;
    [[nodiscard]] bool sourceRolesAffectStructure(const QList<int>& roles) const;
    void forwardSourceDataChanged(const QModelIndex& topLeft, const QModelIndex& bottomRight, const QList<int>& roles);
    void reconcileSourceRows(QList<int> sourceRows);
    void reconcileBlockModel(QAbstractItemModel* blockModel);
    void insertSourceRows(const QModelIndex& parent, int first, int last);
    void removeSourceRows(const QModelIndex& parent, int first, int last);
    [[nodiscard]] QList<ViewportRow> viewportRowsForSourceRow(int sourceRow);
    void connectBlockModel(QAbstractItemModel* blockModel, int sourceRow);
    void disconnectBlockModelSourceRow(QAbstractItemModel* blockModel, int sourceRow);
    void disconnectBlockModels();
    void reconnectBlockModels();
    void reconnectSourceRoles();
    void disconnectModels();
    void rebuild();

    QPointer<QAbstractItemModel> sourceModel_;
    QHash<QByteArray, int> sourceRolesByName_;
    QVector<QMetaObject::Connection> sourceConnections_;
    QHash<QAbstractItemModel*, BlockModelSubscription> blockSubscriptions_;
    QList<ViewportRow> rows_;
    int entryIdRole_ = -1;
    int markupDocumentRole_ = -1;
    int detailRowRole_ = -1;
    int turnExpandedRole_ = -1;
    int revision_ = 0;
};
