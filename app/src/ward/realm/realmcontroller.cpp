// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/realm/realmcontroller.h"

#include "ward/coreffi.h"

#include <QDir>
#include <QFileInfo>
#include <QMetaObject>
#include <QQmlEngine>
#include <QThread>
#include <QWindow>

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

QUrl
RealmController::bundleUrl() const
{
    return bundleUrl_;
}

void
RealmController::setBundleUrl(const QUrl& bundleUrl)
{
    if (!canSelectBundle()) {
        setErrorMessage(tr("Stop the realm before changing its bundle."));
        return;
    }

    QUrl normalizedUrl;
    if (!bundleUrl.isEmpty()) {
        if (!bundleUrl.isLocalFile()) {
            setErrorMessage(tr("The realm bundle must be a local folder."));
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
            return tr("Not opened");
        case Stopped:
            return tr("Stopped");
        case Running:
            return tr("Running");
        case Paused:
            return tr("Paused");
        case Error:
            return tr("Error");
        case Starting:
            return tr("Starting…");
        case Pausing:
            return tr("Pausing…");
        case Resuming:
            return tr("Resuming…");
        case Stopping:
            return tr("Stopping…");
        case Saving:
            return tr("Saving…");
        case Restoring:
            return tr("Restoring…");
        case Suspended:
            return tr("Suspended");
    }
    return tr("Unknown");
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
    beginCommand();
    ward_core_realm_start(realm_);
}

void
RealmController::pause()
{
    if (!canPause())
        return;
    beginCommand();
    ward_core_realm_pause(realm_);
}

void
RealmController::resume()
{
    if (!canResume())
        return;
    beginCommand();
    ward_core_realm_resume(realm_);
}

void
RealmController::requestStop()
{
    if (!canRequestStop())
        return;
    beginCommand();
    ward_core_realm_request_stop(realm_);
}

void
RealmController::forceStop()
{
    if (!canForceStop())
        return;
    beginCommand();
    ward_core_realm_force_stop(realm_);
}

void
RealmController::suspend()
{
    if (!canSuspend())
        return;
    beginCommand();
    ward_core_realm_suspend(realm_);
}

void
RealmController::restore()
{
    if (!ensureRealm() || !canRestore())
        return;
    beginCommand();
    ward_core_realm_restore(realm_);
}

void
RealmController::discardSavedState()
{
    if (!canDiscardSavedState())
        return;
    beginCommand();
    ward_core_realm_discard_saved_state(realm_);
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
    if (nativeView == nullptr) {
        QString message = tr("The Realm display could not be attached.");
        if (error != nullptr) {
            const char* nativeMessage = ward_core_error_message(error);
            if (nativeMessage != nullptr)
                message = QString::fromUtf8(nativeMessage);
            ward_core_error_destroy(error);
        }
        setErrorMessage(message);
        return false;
    }

    std::unique_ptr<QWindow> displayWindow(QWindow::fromWinId(reinterpret_cast<WId>(nativeView)));
    if (displayWindow == nullptr) {
        ward_core_realm_detach_display(realm_);
        setErrorMessage(tr("Qt could not embed the Realm display."));
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
RealmController::handleRealmEvent(void* context, const WardRealmStatus* status, const char* errorMessage)
{
    if (context == nullptr || status == nullptr)
        return;

    auto* callbackContext = static_cast<RealmCallbackContext*>(context);
    RealmController* controller = callbackContext->controller;
    const std::uint64_t generation = callbackContext->generation;
    const WardRealmStatus statusCopy = *status;
    const QString errorCopy = errorMessage != nullptr ? QString::fromUtf8(errorMessage) : QString();
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
        setErrorMessage(tr("Choose an installed realm bundle first."));
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
    if (realm_ != nullptr) {
        emit statusChanged();
        return true;
    }

    QString message = tr("The realm bundle could not be opened.");
    if (error != nullptr) {
        const char* nativeMessage = ward_core_error_message(error);
        if (nativeMessage != nullptr)
            message = QString::fromUtf8(nativeMessage);
        ward_core_error_destroy(error);
    }
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
}

void
RealmController::beginCommand()
{
    setErrorMessage({});
    commandPending_ = true;
    emit statusChanged();
}

void
RealmController::applyStatus(std::uint64_t generation, WardRealmStatus status, const QString& errorMessage)
{
    if (generation != generation_)
        return;

    switch (status.state) {
        case WardRealmStateStopped:
            state_ = Stopped;
            break;
        case WardRealmStateRunning:
            state_ = Running;
            break;
        case WardRealmStatePaused:
            state_ = Paused;
            break;
        case WardRealmStateError:
            state_ = Error;
            break;
        case WardRealmStateStarting:
            state_ = Starting;
            break;
        case WardRealmStatePausing:
            state_ = Pausing;
            break;
        case WardRealmStateResuming:
            state_ = Resuming;
            break;
        case WardRealmStateStopping:
            state_ = Stopping;
            break;
        case WardRealmStateSaving:
            state_ = Saving;
            break;
        case WardRealmStateRestoring:
            state_ = Restoring;
            break;
        case WardRealmStateSuspended:
            state_ = Suspended;
            break;
        default:
            state_ = Error;
            break;
    }

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
}

void
RealmController::setErrorMessage(const QString& errorMessage)
{
    if (errorMessage_ == errorMessage)
        return;
    errorMessage_ = errorMessage;
    emit errorMessageChanged();
}
