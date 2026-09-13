// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "terminalview.h"

#include <QCoreApplication>
#include <QKeyEvent>
#include <QScopeGuard>
#include <contour/display/TerminalDisplay.hpp>
#include <contour/session/TerminalSession.hpp>

namespace {
class EmbeddedDisplay : public contour::display::TerminalDisplay
{
  public:
    EmbeddedDisplay() = default;

  protected:
    bool event(QEvent* event) override
    {
        if (event->type() == QEvent::ShortcutOverride) {
            auto* key = static_cast<QKeyEvent*>(event);
            // Keep application Command shortcuts available while the terminal has focus.
            if (key->modifiers().testFlag(Qt::ControlModifier) && key->key() != Qt::Key_C && key->key() != Qt::Key_V) {
                event->ignore();
                return false;
            }
            event->accept();
            return true;
        }
        if (auto* input = dynamic_cast<QInputEvent*>(event)) {
            // Qt swaps Command and Control on macOS; Contour expects physical modifiers.
            // Restore the event before it can bubble back into the host UI.
            const auto original = input->modifiers();
            auto modifiers = original & ~(Qt::ControlModifier | Qt::MetaModifier);
            if (original.testFlag(Qt::ControlModifier))
                modifiers |= Qt::MetaModifier;
            if (original.testFlag(Qt::MetaModifier))
                modifiers |= Qt::ControlModifier;
            input->setModifiers(modifiers);
            const auto restore = qScopeGuard([input, original] { input->setModifiers(original); });
            return TerminalDisplay::event(event);
        }
        return TerminalDisplay::event(event);
    }
};
}

TerminalView::TerminalView(QQuickItem* parent)
  : QQuickItem(parent)
  , display_(nullptr)
{
    setClip(true);
    setFlag(ItemIsFocusScope);
    // Attach after construction so Contour observes windowChanged even when this view
    // is created directly inside an already-visible window.
    display_ = new EmbeddedDisplay;
    display_->setParent(this);
    display_->setParentItem(this);
    // Clicking the display must reactivate its enclosing focus scopes after a host input took focus.
    display_->setFocusPolicy(Qt::ClickFocus);
    display_->setFocus(true);
}

TerminalView::~TerminalView() = default;

QObject*
TerminalView::session() const
{
    return session_;
}

void
TerminalView::setSession(QObject* session)
{
    if (session_ == session)
        return;
    session_ = session;
    // Contour declares TerminalSession as a Qt plugin interface. Its qobject_cast specialization
    // looks for that interface IID instead of the concrete QObject type.
    static_cast<EmbeddedDisplay*>(display_)->setSession(dynamic_cast<contour::session::TerminalSession*>(session));
    emit sessionChanged();
}

void
TerminalView::focusTerminal()
{
    if (isVisible() && session_)
        display_->forceActiveFocus(Qt::OtherFocusReason);
}

void
TerminalView::geometryChange(const QRectF& newGeometry, const QRectF& oldGeometry)
{
    QQuickItem::geometryChange(newGeometry, oldGeometry);
    display_->setSize(newGeometry.size());
}
