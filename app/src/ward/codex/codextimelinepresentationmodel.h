// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "persistentstringroleindex.h"

#include <QAbstractListModel>
#include <QHash>
#include <QMetaObject>
#include <QPointer>
#include <QSet>
#include <QString>
#include <QVector>
#include <QtQmlIntegration/qqmlintegration.h>

#include <memory>

class TimelineExpansionInvalidationModel;
class TimelinePresentationFilterModel;
class CodexTimelinePresentationModelTest;

class CodexTimelinePresentationModel : public QAbstractListModel
{
    Q_OBJECT
    Q_PROPERTY(QAbstractItemModel* sourceModel READ sourceModel WRITE setSourceModel NOTIFY sourceModelChanged)
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
    ~CodexTimelinePresentationModel() override;

    void setSourceModel(QAbstractItemModel* model);
    [[nodiscard]] QAbstractItemModel* sourceModel() const;

    [[nodiscard]] int totalRowCount() const;
    [[nodiscard]] int revision() const;
    [[nodiscard]] int rowCount(const QModelIndex& parent = {}) const override;
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
    void sourceModelChanged();

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
    [[nodiscard]] bool acceptsSourceRow(int sourceRow) const;
    [[nodiscard]] int chooseInvalidationRole(QAbstractItemModel* model) const;
    void reconnectRoles(QAbstractItemModel* model);
    void recomputeMetadata(QAbstractItemModel* model);
    void refreshChangedRows(QAbstractItemModel* model,
                            const QModelIndex& topLeft,
                            const QModelIndex& bottomRight,
                            const QList<int>& roles);
    void recomputeTurnPresentation(const QString& turnId);
    void connectSourceSignals(QAbstractItemModel* model);
    void connectSourceDestructionSignal(QAbstractItemModel* model);
    void disconnectSourceSignals();
    void connectFilterSignals();
    void invalidateTurns(const QSet<QString>& turnIds);
    void publishExpansionChange(const QString& turnId);
    void advanceRevision();
    void refreshEntryIndexes(const QModelIndex& topLeft, const QModelIndex& bottomRight, const QList<int>& roles);

    QHash<QByteArray, int> rolesByName_;
    PersistentStringRoleIndex entryIndex_;
    QSet<QString> expandedTurns_;
    QHash<QString, QVector<int>> rowsByTurn_;
    QVector<QMetaObject::Connection> sourceConnections_;
    QVector<RowMetadata> metadata_;
    std::unique_ptr<TimelineExpansionInvalidationModel> expansionInvalidationModel_;
    std::unique_ptr<TimelinePresentationFilterModel> filterModel_;
    QPointer<QAbstractItemModel> presentationSourceModel_;
    int turnIdRole_ = -1;
    int activityGroupRole_ = -1;
    int standaloneActivityRole_ = -1;
    int fromUserRole_ = -1;
    int commentaryRole_ = -1;
    int finalAnswerRole_ = -1;
    mutable int filterEvaluationCount_ = 0;
    int revision_ = 0;
    bool forwardingLayoutReset_ = false;

    friend class CodexTimelinePresentationModelTest;
};
