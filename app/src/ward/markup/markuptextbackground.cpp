// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "markuptextbackground.h"

#include <QAbstractTextDocumentLayout>
#include <QMetaProperty>
#include <QQuickTextDocument>
#include <QTextBlock>
#include <QTextDocument>
#include <QTextLayout>

namespace {
void
disconnectAll(QList<QMetaObject::Connection>& connections)
{
    for (const auto& connection : connections)
        QObject::disconnect(connection);
    connections.clear();
}
}

MarkupTextBackground::MarkupTextBackground(QQuickItem* parent, std::initializer_list<const char*> additionalProperties)
  : QQuickItem(parent)
  , observedProperties_{
      "cursorRectangle", "contentWidth", "contentHeight", "textDocument", "topPadding", "leftPadding"
  }
{
    for (const auto* name : additionalProperties)
        observedProperties_.append(name);
}

QQuickItem*
MarkupTextBackground::textEdit() const
{
    return textEdit_;
}

void
MarkupTextBackground::setTextEdit(QQuickItem* item)
{
    if (textEdit_ == item)
        return;
    disconnectAll(itemConnections_);
    textEdit_ = item;
    if (item) {
        const auto slot = metaObject()->method(metaObject()->indexOfSlot("scheduleLayout()"));
        for (const auto& name : observedProperties_) {
            const auto property = item->metaObject()->property(item->metaObject()->indexOfProperty(name.constData()));
            if (property.hasNotifySignal())
                itemConnections_.append(connect(item, property.notifySignal(), this, slot));
        }
        itemConnections_.append(connect(item, &QObject::destroyed, this, [this] {
            scheduleLayout();
            emit textEditChanged();
        }));
    }
    scheduleLayout();
    emit textEditChanged();
}

void
MarkupTextBackground::scheduleLayout()
{
    polish();
}

void
MarkupTextBackground::observeDocument()
{
    auto* quick = textEdit_ ? textEdit_->property("textDocument").value<QQuickTextDocument*>() : nullptr;
    auto* document = quick ? quick->textDocument() : nullptr;
    auto* layout = document ? document->documentLayout() : nullptr;
    if (quickDocument_ == quick && document_ == document && layout_ == layout)
        return;
    disconnectAll(documentConnections_);
    quickDocument_ = quick;
    document_ = document;
    layout_ = layout;
    if (quick) {
        documentConnections_.append(
          connect(quick, &QQuickTextDocument::textDocumentChanged, this, &MarkupTextBackground::scheduleLayout));
        documentConnections_.append(connect(quick, &QObject::destroyed, this, &MarkupTextBackground::scheduleLayout));
    }
    if (document) {
        documentConnections_.append(
          connect(document, &QTextDocument::contentsChanged, this, &MarkupTextBackground::scheduleLayout));
        documentConnections_.append(
          connect(document, &QTextDocument::documentLayoutChanged, this, &MarkupTextBackground::scheduleLayout));
        documentConnections_.append(
          connect(document, &QObject::destroyed, this, &MarkupTextBackground::scheduleLayout));
    }
    if (layout) {
        documentConnections_.append(
          connect(layout, &QAbstractTextDocumentLayout::update, this, &MarkupTextBackground::scheduleLayout));
        documentConnections_.append(
          connect(layout, &QAbstractTextDocumentLayout::updateBlock, this, &MarkupTextBackground::scheduleLayout));
        documentConnections_.append(connect(
          layout, &QAbstractTextDocumentLayout::documentSizeChanged, this, &MarkupTextBackground::scheduleLayout));
    }
}

std::optional<MarkupTextBackground::DocumentGeometry>
MarkupTextBackground::documentGeometry()
{
    observeDocument();
    if (!textEdit_ || !document_)
        return std::nullopt;
    document_->documentLayout()->documentSize();
    const auto* first = document_->firstBlock().layout();
    if (!first || first->lineCount() == 0)
        return std::nullopt;
    const auto line = first->lineAt(0);
    QRectF cursor;
    if (!QMetaObject::invokeMethod(textEdit_, "positionToRectangle", Q_RETURN_ARG(QRectF, cursor), Q_ARG(int, 0)))
        return std::nullopt;
    const auto origin =
      textEdit_->mapToItem(this, cursor.topLeft()) - first->position() - QPointF(line.cursorToX(0), line.y());
    return DocumentGeometry{ document_, origin };
}

void
MarkupTextBackground::geometryChange(const QRectF& geometry, const QRectF& previous)
{
    QQuickItem::geometryChange(geometry, previous);
    scheduleLayout();
}
