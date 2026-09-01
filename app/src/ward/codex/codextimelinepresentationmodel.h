// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QHash>
#include <QMetaObject>
#include <QSet>
#include <QSortFilterProxyModel>
#include <QString>
#include <QVector>
#include <QtQmlIntegration/qqmlintegration.h>

class CodexTimelinePresentationModel : public QSortFilterProxyModel
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(int totalRowCount READ totalRowCount NOTIFY statisticsChanged)
    Q_PROPERTY(int revision READ revision NOTIFY revisionChanged)

  public:
    enum Role
    {
        DetailRowRole = Qt::UserRole + 0x2000,
        FirstDetailInTurnRole,
        DetailCountInTurnRole,
        TurnExpandedRole,
    };

    explicit CodexTimelinePresentationModel(QObject* parent = nullptr);

    void setSourceModel(QAbstractItemModel* model) override;

    [[nodiscard]] int totalRowCount() const;
    [[nodiscard]] int revision() const;
    [[nodiscard]] QVariant data(const QModelIndex& index, int role) const override;
    [[nodiscard]] QHash<int, QByteArray> roleNames() const override;

    Q_INVOKABLE [[nodiscard]] QVariant valueAt(int row, const QString& roleName) const;
    Q_INVOKABLE [[nodiscard]] QString entryIdAt(int row) const;
    Q_INVOKABLE [[nodiscard]] int indexOfEntryId(const QString& entryId) const;
    Q_INVOKABLE [[nodiscard]] bool turnExpanded(const QString& turnId) const;
    Q_INVOKABLE void setTurnExpanded(const QString& turnId, bool expanded);
    Q_INVOKABLE void toggleTurn(const QString& turnId);
    Q_INVOKABLE void clearExpandedTurns();

  signals:
    void statisticsChanged();
    void revisionChanged();

  protected:
    [[nodiscard]] bool filterAcceptsRow(int sourceRow, const QModelIndex& sourceParent) const override;

  private:
    struct RowMetadata
    {
        QString turnId;
        bool detail = false;
        bool firstDetailInTurn = false;
        int detailCountInTurn = 0;
    };

    [[nodiscard]] int roleForName(const QByteArray& roleName) const;
    [[nodiscard]] QVariant sourceValue(int sourceRow, int role) const;
    [[nodiscard]] RowMetadata sourceRowMetadata(QAbstractItemModel* model, int sourceRow) const;
    [[nodiscard]] bool rolesAffectPresentation(const QList<int>& roles) const;
    void reconnectRoles(QAbstractItemModel* model);
    void recomputeMetadata(QAbstractItemModel* model);
    void refreshChangedRows(QAbstractItemModel* model,
                            const QModelIndex& topLeft,
                            const QModelIndex& bottomRight,
                            const QList<int>& roles);
    void recomputeTurnPresentation(const QString& turnId);
    void connectSourceSignals(QAbstractItemModel* model);
    void disconnectSourceSignals();
    void publishExpansionChange(const QString& turnId);
    void advanceRevision();

    QHash<QByteArray, int> rolesByName_;
    QSet<QString> expandedTurns_;
    QHash<QString, QVector<int>> rowsByTurn_;
    QVector<QMetaObject::Connection> sourceConnections_;
    QVector<RowMetadata> metadata_;
    int turnIdRole_ = -1;
    int activityGroupRole_ = -1;
    int standaloneActivityRole_ = -1;
    int fromUserRole_ = -1;
    int commentaryRole_ = -1;
    int finalAnswerRole_ = -1;
    int revision_ = 0;
};
