// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/realm/realmcontroller.h"

#include "ward/coreffierror.h"

#include <ward_core.h>

#include <QDir>
#include <QFileInfo>
#include <QMetaObject>
#include <QQmlEngine>
#include <QThread>
#include <QWindow>
#include <QtTranslation>

#include <utility>

struct RealmCallbackContext
{
    RealmController* controller;
    std::uint64_t generation;
};

RealmController::RealmController(QObject* parent)
  : QObject(parent)
{
}

RealmController::~RealmController()
{
    ++generation_;
    detachDisplay();
    if (realm_ != nullptr)
        ward_core_realm_destroy(std::exchange(realm_, nullptr));
    callbackContext_.reset();
}

void
RealmController::retranslate()
{
    emit stateTextChanged();
}

QUrl
RealmController::bundleUrl() const
{
    return bundleUrl_;
}

void
RealmController::setBundleUrl(const QUrl& bundleUrl)
{
    if (!canSelectBundle()) {
        setErrorMessage(/*% "Stop the realm before changing its bundle." */ qtTrId(
          "craftward.realm.error.stop_before_bundle_change"));
        return;
    }

    QUrl normalizedUrl;
    if (!bundleUrl.isEmpty()) {
        if (!bundleUrl.isLocalFile()) {
            setErrorMessage(
              /*% "The realm bundle must be a local folder." */ qtTrId("craftward.realm.error.bundle_not_local"));
            return;
        }
        normalizedUrl = QUrl::fromLocalFile(QDir::cleanPath(bundleUrl.toLocalFile()));
    }
    if (bundleUrl_ == normalizedUrl)
        return;

    destroyRealm();
    bundleUrl_ = normalizedUrl;
    setErrorMessage({});
    emit bundleChanged();
    emit statusChanged();
    if (!bundleUrl_.isEmpty())
        (void)ensureRealm();
}

QString
RealmController::bundlePath() const
{
    return bundleUrl_.toLocalFile();
}

QString
RealmController::displayName() const
{
    const QFileInfo bundle(bundlePath());
    const QString baseName = bundle.completeBaseName();
    return !baseName.isEmpty() ? baseName : bundle.fileName();
}

RealmController::State
RealmController::state() const
{
    return state_;
}

QString
RealmController::stateText() const
{
    switch (state_) {
        case Closed:
            return /*% "Not opened" */ qtTrId("craftward.realm.state.closed");
        case Stopped:
            return /*% "Stopped" */ qtTrId("craftward.realm.state.stopped");
        case Running:
            return /*% "Running" */ qtTrId("craftward.realm.state.running");
        case Paused:
            return /*% "Paused" */ qtTrId("craftward.realm.state.paused");
        case Error:
            return /*% "Error" */ qtTrId("craftward.realm.state.error");
        case Starting:
            return /*% "Starting…" */ qtTrId("craftward.realm.state.starting");
        case Pausing:
            return /*% "Pausing…" */ qtTrId("craftward.realm.state.pausing");
        case Resuming:
            return /*% "Resuming…" */ qtTrId("craftward.realm.state.resuming");
        case Stopping:
            return /*% "Stopping…" */ qtTrId("craftward.realm.state.stopping");
        case Saving:
            return /*% "Saving…" */ qtTrId("craftward.realm.state.saving");
        case Restoring:
            return /*% "Restoring…" */ qtTrId("craftward.realm.state.restoring");
        case Suspended:
            return /*% "Suspended" */ qtTrId("craftward.realm.state.suspended");
    }
    return /*% "Unknown" */ qtTrId("craftward.realm.state.unknown");
}

QString
RealmController::errorMessage() const
{
    return errorMessage_;
}

QWindow*
RealmController::displayWindow() const
{
    return displayWindow_.get();
}

bool
RealmController::busy() const
{
    return commandPending_ || state_ == Starting || state_ == Pausing || state_ == Resuming || state_ == Stopping ||
           state_ == Saving || state_ == Restoring;
}

bool
RealmController::canSelectBundle() const
{
    return !commandPending_ && (realm_ == nullptr || state_ == Stopped || state_ == Suspended ||
                                (state_ == Error && !backendCanForceStop_));
}

bool
RealmController::canStart() const
{
    if (commandPending_ || bundleUrl_.isEmpty())
        return false;
    return realm_ == nullptr || backendCanStart_;
}

bool
RealmController::canPause() const
{
    return !commandPending_ && realm_ != nullptr && backendCanPause_;
}

bool
RealmController::canResume() const
{
    return !commandPending_ && realm_ != nullptr && backendCanResume_;
}

bool
RealmController::canRequestStop() const
{
    return !commandPending_ && realm_ != nullptr && backendCanRequestStop_;
}

bool
RealmController::canForceStop() const
{
    return !commandPending_ && realm_ != nullptr && backendCanForceStop_;
}

bool
RealmController::canSuspend() const
{
    return !commandPending_ && realm_ != nullptr && backendCanSuspend_;
}

bool
RealmController::canRestore() const
{
    return !commandPending_ && realm_ != nullptr && backendCanRestore_;
}

bool
RealmController::canDiscardSavedState() const
{
    return !commandPending_ && realm_ != nullptr && backendCanDiscardSavedState_;
}

bool
RealmController::requiresStopBeforeExit() const
{
    return realm_ != nullptr && (busy() || backendCanForceStop_ || state_ == Running || state_ == Paused);
}

void
RealmController::start()
{
    if (!ensureRealm() || !canStart())
        return;
    queueCommand(ward_core_realm_start_async);
}

void
RealmController::pause()
{
    if (!canPause())
        return;
    queueCommand(ward_core_realm_pause_async);
}

void
RealmController::resume()
{
    if (!canResume())
        return;
    queueCommand(ward_core_realm_resume_async);
}

void
RealmController::requestStop()
{
    if (!canRequestStop())
        return;
    queueCommand(ward_core_realm_request_stop_async);
}

void
RealmController::forceStop()
{
    if (!canForceStop())
        return;
    queueCommand(ward_core_realm_force_stop_async);
}

void
RealmController::suspend()
{
    if (!canSuspend())
        return;
    queueCommand(ward_core_realm_suspend_async);
}

void
RealmController::restore()
{
    if (!ensureRealm() || !canRestore())
        return;
    queueCommand(ward_core_realm_restore_async);
}

void
RealmController::discardSavedState()
{
    if (!canDiscardSavedState())
        return;
    queueCommand(ward_core_realm_discard_saved_state_async);
}

bool
RealmController::attachDisplay()
{
    if (displayWindow_)
        return true;
    if (!ensureRealm())
        return false;

    WardError* error = nullptr;
    void* nativeView = ward_core_realm_attach_display(realm_, &error);
    const QString coreErrorMessage = ward::coreffi::takeErrorMessage(error);
    if (nativeView == nullptr) {
        const QString message =
          coreErrorMessage.isEmpty()
            ? /*% "The Realm display could not be attached." */ qtTrId("craftward.realm.error.display_attach")
            : coreErrorMessage;
        setErrorMessage(message);
        return false;
    }

    std::unique_ptr<QWindow> displayWindow(QWindow::fromWinId(reinterpret_cast<WId>(nativeView)));
    if (displayWindow == nullptr) {
        ward_core_realm_detach_display(realm_);
        setErrorMessage(/*% "Qt could not embed the Realm display." */ qtTrId("craftward.realm.error.display_embed"));
        return false;
    }

    QQmlEngine::setObjectOwnership(displayWindow.get(), QQmlEngine::CppOwnership);
    displayWindow_ = std::move(displayWindow);
    setErrorMessage({});
    emit displayWindowChanged();
    return true;
}

void
RealmController::detachDisplay()
{
    if (displayWindow_ == nullptr)
        return;

    std::unique_ptr<QWindow> displayWindow = std::move(displayWindow_);
    emit displayWindowChanged();
    displayWindow.reset();
    if (realm_ != nullptr)
        ward_core_realm_detach_display(realm_);
}

void
RealmController::clearError()
{
    setErrorMessage({});
}

void
RealmController::handleRealmEvent(void* context, const WardRealmEvent* event)
{
    if (context == nullptr || event == nullptr)
        return;

    auto* callbackContext = static_cast<RealmCallbackContext*>(context);
    RealmController* controller = callbackContext->controller;
    const std::uint64_t generation = callbackContext->generation;
    const WardRealmStatus statusCopy = event->status;
    const QString errorCopy = event->error_message != nullptr ? QString::fromUtf8(event->error_message) : QString();
    auto apply = [controller, generation, statusCopy, errorCopy] {
        if (controller->generation_ == generation)
            controller->applyStatus(generation, statusCopy, errorCopy);
    };

    if (QThread::currentThread() == controller->thread()) {
        apply();
    } else {
        QMetaObject::invokeMethod(controller, std::move(apply), Qt::QueuedConnection);
    }
}

bool
RealmController::ensureRealm()
{
    if (realm_ != nullptr)
        return true;
    if (bundleUrl_.isEmpty()) {
        setErrorMessage(
          /*% "Choose an installed realm bundle first." */ qtTrId("craftward.realm.error.bundle_required"));
        return false;
    }

    setErrorMessage({});
    ++generation_;
    callbackContext_ = std::make_unique<RealmCallbackContext>(RealmCallbackContext{
      .controller = this,
      .generation = generation_,
    });

    WardError* error = nullptr;
    const QByteArray bundlePath = bundleUrl_.toLocalFile().toUtf8();
    realm_ = ward_core_realm_open(bundlePath.constData(), handleRealmEvent, callbackContext_.get(), &error);
    const QString coreErrorMessage = ward::coreffi::takeErrorMessage(error);
    if (realm_ != nullptr) {
        emit statusChanged();
        return true;
    }

    const QString message =
      coreErrorMessage.isEmpty()
        ? /*% "The realm bundle could not be opened." */ qtTrId("craftward.realm.error.bundle_open")
        : coreErrorMessage;
    callbackContext_.reset();
    setErrorMessage(message);
    return false;
}

void
RealmController::destroyRealm()
{
    ++generation_;
    detachDisplay();
    if (realm_ != nullptr)
        ward_core_realm_destroy(std::exchange(realm_, nullptr));
    callbackContext_.reset();
    const bool stateChanged = state_ != Closed;
    state_ = Closed;
    commandPending_ = false;
    backendCanStart_ = false;
    backendCanPause_ = false;
    backendCanResume_ = false;
    backendCanRequestStop_ = false;
    backendCanForceStop_ = false;
    backendCanSuspend_ = false;
    backendCanRestore_ = false;
    backendCanDiscardSavedState_ = false;
    if (stateChanged)
        emit stateTextChanged();
}

void
RealmController::beginCommand()
{
    setErrorMessage({});
    commandPending_ = true;
    emit statusChanged();
}

void
RealmController::queueCommand(RealmCommand command)
{
    beginCommand();
    WardError* error = nullptr;
    if (command(realm_, &error))
        return;

    commandPending_ = false;
    QString message = ward::coreffi::takeErrorMessage(error);
    if (message.isEmpty())
        message = /*% "The Realm command could not be queued." */ qtTrId("craftward.realm.error.command_queue");
    setErrorMessage(message);
    emit statusChanged();
}

void
RealmController::applyStatus(std::uint64_t generation, WardRealmStatus status, const QString& errorMessage)
{
    if (generation != generation_)
        return;

    State nextState = Error;
    switch (status.state) {
        case WardRealmStateStopped:
            nextState = Stopped;
            break;
        case WardRealmStateRunning:
            nextState = Running;
            break;
        case WardRealmStatePaused:
            nextState = Paused;
            break;
        case WardRealmStateError:
            nextState = Error;
            break;
        case WardRealmStateStarting:
            nextState = Starting;
            break;
        case WardRealmStatePausing:
            nextState = Pausing;
            break;
        case WardRealmStateResuming:
            nextState = Resuming;
            break;
        case WardRealmStateStopping:
            nextState = Stopping;
            break;
        case WardRealmStateSaving:
            nextState = Saving;
            break;
        case WardRealmStateRestoring:
            nextState = Restoring;
            break;
        case WardRealmStateSuspended:
            nextState = Suspended;
            break;
        default:
            break;
    }
    const bool stateChanged = state_ != nextState;
    state_ = nextState;

    commandPending_ = false;
    backendCanStart_ = status.can_start;
    backendCanPause_ = status.can_pause;
    backendCanResume_ = status.can_resume;
    backendCanRequestStop_ = status.can_request_stop;
    backendCanForceStop_ = status.can_force_stop;
    backendCanSuspend_ = status.can_suspend;
    backendCanRestore_ = status.can_restore;
    backendCanDiscardSavedState_ = status.can_discard_saved_state;
    if (!errorMessage.isEmpty())
        setErrorMessage(errorMessage);
    emit statusChanged();
    if (stateChanged)
        emit stateTextChanged();
}

void
RealmController::setErrorMessage(const QString& errorMessage)
{
    if (errorMessage_ == errorMessage)
        return;
    errorMessage_ = errorMessage;
    emit errorMessageChanged();
}
