// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QObject>
#include <QString>
#include <QUrl>
#include <QWindow>
#include <QtQml/qqmlregistration.h>

#include <cstdint>
#include <memory>

struct RealmCallbackContext;
struct WardError;
struct WardRealm;
struct WardRealmEvent;
struct WardRealmStatus;

class RealmController : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("RealmController is provided by the application.")
    Q_PROPERTY(QUrl bundleUrl READ bundleUrl WRITE setBundleUrl NOTIFY bundleChanged)
    Q_PROPERTY(QString bundlePath READ bundlePath NOTIFY bundleChanged)
    Q_PROPERTY(QString displayName READ displayName NOTIFY bundleChanged)
    Q_PROPERTY(State state READ state NOTIFY statusChanged)
    Q_PROPERTY(QString stateText READ stateText NOTIFY stateTextChanged)
    Q_PROPERTY(QString errorMessage READ errorMessage NOTIFY errorMessageChanged)
    Q_PROPERTY(QWindow* displayWindow READ displayWindow NOTIFY displayWindowChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY statusChanged)
    Q_PROPERTY(bool canSelectBundle READ canSelectBundle NOTIFY statusChanged)
    Q_PROPERTY(bool canStart READ canStart NOTIFY statusChanged)
    Q_PROPERTY(bool canPause READ canPause NOTIFY statusChanged)
    Q_PROPERTY(bool canResume READ canResume NOTIFY statusChanged)
    Q_PROPERTY(bool canRequestStop READ canRequestStop NOTIFY statusChanged)
    Q_PROPERTY(bool canForceStop READ canForceStop NOTIFY statusChanged)
    Q_PROPERTY(bool canSuspend READ canSuspend NOTIFY statusChanged)
    Q_PROPERTY(bool canRestore READ canRestore NOTIFY statusChanged)
    Q_PROPERTY(bool canDiscardSavedState READ canDiscardSavedState NOTIFY statusChanged)
    Q_PROPERTY(bool requiresStopBeforeExit READ requiresStopBeforeExit NOTIFY statusChanged)

  public:
    enum State
    {
        Closed = -1,
        Stopped,
        Running,
        Paused,
        Error,
        Starting,
        Pausing,
        Resuming,
        Stopping,
        Saving,
        Restoring,
        Suspended,
    };
    Q_ENUM(State)

    explicit RealmController(QObject* parent = nullptr);
    ~RealmController() override;

    [[nodiscard]] QUrl bundleUrl() const;
    void setBundleUrl(const QUrl& bundleUrl);

    [[nodiscard]] QString bundlePath() const;
    [[nodiscard]] QString displayName() const;
    [[nodiscard]] State state() const;
    [[nodiscard]] QString stateText() const;
    [[nodiscard]] QString errorMessage() const;
    [[nodiscard]] QWindow* displayWindow() const;

    [[nodiscard]] bool busy() const;
    [[nodiscard]] bool canSelectBundle() const;
    [[nodiscard]] bool canStart() const;
    [[nodiscard]] bool canPause() const;
    [[nodiscard]] bool canResume() const;
    [[nodiscard]] bool canRequestStop() const;
    [[nodiscard]] bool canForceStop() const;
    [[nodiscard]] bool canSuspend() const;
    [[nodiscard]] bool canRestore() const;
    [[nodiscard]] bool canDiscardSavedState() const;
    [[nodiscard]] bool requiresStopBeforeExit() const;

    Q_INVOKABLE void start();
    Q_INVOKABLE void pause();
    Q_INVOKABLE void resume();
    Q_INVOKABLE void requestStop();
    Q_INVOKABLE void forceStop();
    Q_INVOKABLE void suspend();
    Q_INVOKABLE void restore();
    Q_INVOKABLE void discardSavedState();
    Q_INVOKABLE bool attachDisplay();
    Q_INVOKABLE void detachDisplay();
    Q_INVOKABLE void clearError();
    void retranslate();

  signals:
    void bundleChanged();
    void statusChanged();
    void stateTextChanged();
    void errorMessageChanged();
    void displayWindowChanged();

  private:
    using RealmCommand = bool (*)(WardRealm* realm, WardError** outputError);

    static void handleRealmEvent(void* context, const WardRealmEvent* event);

    [[nodiscard]] bool ensureRealm();
    void destroyRealm();
    void beginCommand();
    void queueCommand(RealmCommand command);
    void applyStatus(std::uint64_t generation, WardRealmStatus status, const QString& errorMessage);
    void setErrorMessage(const QString& errorMessage);

    QUrl bundleUrl_;
    WardRealm* realm_ = nullptr;
    std::unique_ptr<QWindow> displayWindow_;
    std::unique_ptr<RealmCallbackContext> callbackContext_;
    std::uint64_t generation_ = 0;
    State state_ = Closed;
    QString errorMessage_;
    bool commandPending_ = false;
    bool backendCanStart_ = false;
    bool backendCanPause_ = false;
    bool backendCanResume_ = false;
    bool backendCanRequestStop_ = false;
    bool backendCanForceStop_ = false;
    bool backendCanSuspend_ = false;
    bool backendCanRestore_ = false;
    bool backendCanDiscardSavedState_ = false;
};
