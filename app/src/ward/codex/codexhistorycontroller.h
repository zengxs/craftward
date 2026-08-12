// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "codexmessagemodel.h"
#include "codexthreadmodel.h"

#include <QObject>
#include <QString>
#include <QtQml/qqmlregistration.h>

#include <cstdint>

class CodexHistoryController : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("CodexHistoryController is provided by the application.")
    Q_PROPERTY(CodexThreadModel* threads READ threads CONSTANT)
    Q_PROPERTY(CodexMessageModel* messages READ messages CONSTANT)
    Q_PROPERTY(QString selectedThreadId READ selectedThreadId NOTIFY selectionChanged)
    Q_PROPERTY(QString selectedThreadTitle READ selectedThreadTitle NOTIFY selectionChanged)
    Q_PROPERTY(QString errorMessage READ errorMessage NOTIFY errorMessageChanged)
    Q_PROPERTY(bool loadingThreads READ loadingThreads NOTIFY loadingChanged)
    Q_PROPERTY(bool loadingConversation READ loadingConversation NOTIFY loadingChanged)

  public:
    explicit CodexHistoryController(QObject* parent = nullptr);
    ~CodexHistoryController() override;

    [[nodiscard]] CodexThreadModel* threads();
    [[nodiscard]] CodexMessageModel* messages();
    [[nodiscard]] QString selectedThreadId() const;
    [[nodiscard]] QString selectedThreadTitle() const;
    [[nodiscard]] QString errorMessage() const;
    [[nodiscard]] bool loadingThreads() const;
    [[nodiscard]] bool loadingConversation() const;

    Q_INVOKABLE void refresh();
    Q_INVOKABLE void selectThread(const QString& threadId, const QString& title);
    Q_INVOKABLE void clearError();

  signals:
    void selectionChanged();
    void errorMessageChanged();
    void loadingChanged();

  private:
    void setErrorMessage(const QString& message);
    void applyThreads(std::uint64_t generation, QList<CodexThreadSummary> threads, const QString& errorMessage);
    void applyConversation(std::uint64_t generation,
                           const QString& threadId,
                           const QString& title,
                           QList<CodexMessage> messages,
                           const QString& errorMessage);

    CodexThreadModel threadModel_;
    CodexMessageModel messageModel_;
    QString selectedThreadId_;
    QString selectedThreadTitle_;
    QString errorMessage_;
    std::uint64_t threadGeneration_ = 0;
    std::uint64_t conversationGeneration_ = 0;
    bool loadingThreads_ = false;
    bool loadingConversation_ = false;
};
