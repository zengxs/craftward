// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/markup/markuptextdocument.h"

#include "document.qpb.h"

#include <QTextBlock>
#include <QTextCursor>
#include <QTextList>
#include <QTextTable>

namespace {
using namespace ward::markup::v1;
using ContainerKind = ContainerKindGadget::ContainerKind;
using TextKind = TextKindGadget::TextKind;
using ColumnAlignment = ColumnAlignmentGadget::ColumnAlignment;

struct TextStyle
{
    QFont font;
    QFont codeFont;
    QColor foreground;
    QColor link;
    QColor background;
};

class SegmentWriter
{
  public:
    SegmentWriter(QTextDocument* document, TextStyle style)
      : cursor_(document)
      , style_(std::move(style))
    {
    }

    void write(const SemanticDocument& segment)
    {
        QTextCharFormat format;
        format.setFont(style_.font);
        format.setForeground(style_.foreground);
        for (const auto& block : segment.blocks()) {
            nodes_ = block.nodes();
            children_ = QList<QList<qsizetype>>(nodes_.size());
            for (qsizetype index = 0; index < nodes_.size(); ++index) {
                if (nodes_[index].hasParentIndex() && nodes_[index].parentIndex() < index)
                    children_[nodes_[index].parentIndex()].append(index);
            }
            for (qsizetype index = 0; index < nodes_.size(); ++index) {
                if (!nodes_[index].hasParentIndex())
                    writeNode(index, format, 0, 0);
            }
        }
    }

  private:
    void beginBlock(int quoteDepth, int listDepth, qreal spacing = 8)
    {
        QTextBlockFormat format;
        format.setLeftMargin(quoteDepth * 14);
        format.setIndent(listDepth);
        format.setTopMargin(blockStarted_ ? spacing : 0);
        if (blockStarted_)
            cursor_.insertBlock(format);
        else
            cursor_.setBlockFormat(format);
        blockStarted_ = true;
    }

    void writeChildren(qsizetype index, const QTextCharFormat& format, int quoteDepth, int listDepth)
    {
        for (qsizetype child : children_[index])
            writeNode(child, format, quoteDepth, listDepth);
    }

    void writeList(qsizetype index, const QTextCharFormat& format, int quoteDepth, int listDepth)
    {
        QTextListFormat listFormat;
        listFormat.setIndent(listDepth + 1);
        listFormat.setStyle(nodes_[index].list().hasStart() ? QTextListFormat::ListDecimal : QTextListFormat::ListDisc);
        listFormat.setStart(nodes_[index].list().hasStart() ? nodes_[index].list().start() : 1);
        QTextList* list = nullptr;
        for (qsizetype item : children_[index]) {
            beginBlock(quoteDepth, 0, 2);
            if (!list)
                list = cursor_.createList(listFormat);
            else
                list->add(cursor_.block());
            bool first = true;
            for (qsizetype child : children_[item]) {
                if (first && nodes_[child].hasContainer() &&
                    nodes_[child].container() == ContainerKind::CONTAINER_KIND_PARAGRAPH)
                    writeChildren(child, format, quoteDepth, listDepth + 1);
                else
                    writeNode(child, format, quoteDepth, listDepth + 1);
                first = false;
            }
        }
    }

    void writeTable(qsizetype index, const QTextCharFormat& format, int quoteDepth, int listDepth)
    {
        const auto alignments = nodes_[index].table().columns();
        if (alignments.isEmpty() || children_[index].isEmpty())
            return;
        if (blockStarted_)
            cursor_.insertBlock();
        QTextBlockFormat boundary;
        boundary.setLineHeight(1, QTextBlockFormat::FixedHeight);
        cursor_.setBlockFormat(boundary);
        QTextTableFormat tableFormat;
        tableFormat.setLeftMargin(quoteDepth * 14 + listDepth * cursor_.document()->indentWidth());
        tableFormat.setWidth(QTextLength(QTextLength::PercentageLength, 100));
        tableFormat.setBorder(0.5);
        tableFormat.setBorderBrush(style_.background.darker(120));
        tableFormat.setBorderStyle(QTextFrameFormat::BorderStyle_Solid);
        tableFormat.setCellPadding(6);
        tableFormat.setCellSpacing(0);
        QList<QTextLength> widths(alignments.size(),
                                  QTextLength(QTextLength::PercentageLength, 100.0 / alignments.size()));
        tableFormat.setColumnWidthConstraints(widths);
        auto* table = cursor_.insertTable(children_[index].size(), alignments.size(), tableFormat);
        for (qsizetype row = 0; row < children_[index].size(); ++row) {
            const qsizetype rowNode = children_[index][row];
            for (qsizetype column = 0; column < children_[rowNode].size() && column < alignments.size(); ++column) {
                auto cell = table->cellAt(row, column);
                cursor_ = cell.firstCursorPosition();
                QTextBlockFormat block;
                if (alignments[column] == ColumnAlignment::COLUMN_ALIGNMENT_CENTER)
                    block.setAlignment(Qt::AlignHCenter);
                else if (alignments[column] == ColumnAlignment::COLUMN_ALIGNMENT_RIGHT)
                    block.setAlignment(Qt::AlignRight);
                cursor_.setBlockFormat(block);
                auto cellText = format;
                if (nodes_[rowNode].tableRowHeader()) {
                    cellText.setFontWeight(QFont::Bold);
                    auto cellFormat = cell.format();
                    cellFormat.setBackground(style_.background);
                    cell.setFormat(cellFormat);
                }
                writeChildren(children_[rowNode][column], cellText, 0, 0);
            }
        }
        cursor_.setPosition(table->lastPosition() + 1);
        cursor_.setBlockFormat(boundary);
        blockStarted_ = true;
    }

    void writeNode(qsizetype index, QTextCharFormat format, int quoteDepth, int listDepth)
    {
        const auto& node = nodes_[index];
        if (node.hasText()) {
            QString text = node.text().value().text();
            switch (node.text().kind()) {
                case TextKind::TEXT_KIND_INLINE_CODE:
                    format.setFontFamilies(style_.codeFont.families());
                    format.setFontFixedPitch(true);
                    format.setBackground(style_.background);
                    break;
                case TextKind::TEXT_KIND_SOFT_BREAK:
                    text = QStringLiteral(" ");
                    break;
                case TextKind::TEXT_KIND_HARD_BREAK:
                    text = QChar::LineSeparator;
                    break;
                default:
                    break;
            }
            cursor_.insertText(text, format);
        } else if (node.hasAnnotation()) {
            format.setForeground(style_.link);
            format.setBackground(style_.background);
            cursor_.insertText(node.annotation().label().text(), format);
        } else if (node.hasTaskChecked()) {
            auto block = cursor_.blockFormat();
            block.setMarker(node.taskChecked() ? QTextBlockFormat::MarkerType::Checked
                                               : QTextBlockFormat::MarkerType::Unchecked);
            cursor_.setBlockFormat(block);
        } else if (node.hasList()) {
            writeList(index, format, quoteDepth, listDepth);
        } else if (node.hasTable()) {
            writeTable(index, format, quoteDepth, listDepth);
        } else if (node.hasRule()) {
            beginBlock(quoteDepth, listDepth);
            auto block = cursor_.blockFormat();
            block.setProperty(QTextFormat::BlockTrailingHorizontalRulerWidth,
                              QVariant::fromValue(QTextLength(QTextLength::PercentageLength, 100)));
            cursor_.setBlockFormat(block);
        } else {
            if (node.hasHeadingLevel()) {
                beginBlock(quoteDepth, listDepth);
                auto block = cursor_.blockFormat();
                block.setHeadingLevel(node.headingLevel());
                cursor_.setBlockFormat(block);
                auto headingFont = style_.font;
                const qreal factor = 1.0 + (7 - node.headingLevel()) * 0.12;
                if (headingFont.pixelSize() > 0)
                    headingFont.setPixelSize(qRound(headingFont.pixelSize() * factor));
                else
                    headingFont.setPointSizeF(headingFont.pointSizeF() * factor);
                headingFont.setBold(true);
                format.setFont(headingFont);
            } else if (node.hasCodeBlock()) {
                beginBlock(quoteDepth, listDepth);
                format.setFont(style_.codeFont);
                format.setBackground(style_.background);
            } else if (node.hasLink()) {
                format.setAnchor(true);
                format.setAnchorHref(node.link().target());
                format.setToolTip(node.link().title());
                format.setForeground(style_.link);
                format.setFontUnderline(true);
            } else if (node.hasContainer()) {
                switch (node.container()) {
                    case ContainerKind::CONTAINER_KIND_PARAGRAPH:
                        beginBlock(quoteDepth, listDepth);
                        break;
                    case ContainerKind::CONTAINER_KIND_BLOCK_QUOTE:
                        ++quoteDepth;
                        break;
                    case ContainerKind::CONTAINER_KIND_EMPHASIS:
                        format.setFontItalic(true);
                        break;
                    case ContainerKind::CONTAINER_KIND_STRONG:
                        format.setFontWeight(QFont::Bold);
                        break;
                    case ContainerKind::CONTAINER_KIND_STRIKETHROUGH:
                        format.setFontStrikeOut(true);
                        break;
                    default:
                        break;
                }
            }
            writeChildren(index, format, quoteDepth, listDepth);
        }
    }

    QTextCursor cursor_;
    QList<SemanticNode> nodes_;
    TextStyle style_;
    QList<QList<qsizetype>> children_;
    bool blockStarted_ = false;
};
}

MarkupTextDocument::MarkupTextDocument(QObject* parent)
  : QObject(parent)
{
    connect(this, &MarkupTextDocument::styleChanged, this, &MarkupTextDocument::render);
}

QQuickTextDocument*
MarkupTextDocument::textDocument() const
{
    return document_;
}

void
MarkupTextDocument::setTextDocument(QQuickTextDocument* document)
{
    if (document_ == document)
        return;
    if (document_)
        disconnect(document_, nullptr, this, nullptr);
    document_ = document;
    if (document_)
        connect(document_, &QQuickTextDocument::textDocumentChanged, this, &MarkupTextDocument::render);
    render();
    emit textDocumentChanged();
}

QVariant
MarkupTextDocument::segment() const
{
    return segment_;
}

void
MarkupTextDocument::setSegment(const QVariant& segment)
{
    if (segment_ == segment)
        return;
    segment_ = segment;
    render();
    emit segmentChanged();
}

void
MarkupTextDocument::classBegin()
{
    complete_ = false;
}

void
MarkupTextDocument::componentComplete()
{
    complete_ = true;
    render();
}

void
MarkupTextDocument::render()
{
    if (!complete_ || !document_ || !document_->textDocument())
        return;
    auto* document = document_->textDocument();
    document->setUndoRedoEnabled(false);
    // TextEdit measures on contentsChanged, so publish only the completed layout.
    // Disabling layout still emits content changes and leaves its height stale.
    QTextCursor transaction(document);
    transaction.beginEditBlock();
    transaction.select(QTextCursor::Document);
    transaction.removeSelectedText();
    transaction.setBlockFormat(QTextBlockFormat());
    transaction.setCharFormat(QTextCharFormat());
    document->setDefaultFont(font_);
    document->setDocumentMargin(0);
    if (segment_.canConvert<SemanticDocument>()) {
        SegmentWriter writer(document, { font_, codeFont_, textColor_, linkColor_, codeBackground_ });
        writer.write(segment_.value<SemanticDocument>());
    }
    transaction.endEditBlock();
}
