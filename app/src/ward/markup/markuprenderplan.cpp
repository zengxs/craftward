// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "markuprenderplan.h"

namespace {
using namespace ward::markup::v1;
using ContainerKind = ContainerKindGadget::ContainerKind;
using TextKind = TextKindGadget::TextKind;
using ColumnAlignment = ColumnAlignmentGadget::ColumnAlignment;

class Projector
{
  public:
    QVariantList project(const SemanticDocument& document)
    {
        for (const auto& block : document.blocks()) {
            blockId_ = block.blockId();
            nodes_ = block.nodes();
            children_ = QList<QList<qsizetype>>(nodes_.size());
            for (qsizetype i = 0; i < nodes_.size(); ++i) {
                if (nodes_[i].hasParentIndex() && nodes_[i].parentIndex() < i)
                    children_[nodes_[i].parentIndex()].append(i);
            }
            for (qsizetype i = 0; i < nodes_.size(); ++i) {
                if (!nodes_[i].hasParentIndex())
                    node(i, {}, 0, 0);
            }
        }
        flush();
        return parts_;
    }

  private:
    QString key(qsizetype index) const
    {
        // Prefix the block length because node identities are block-scoped.
        return QString::number(blockId_.size()) + QLatin1Char(':') + blockId_ + nodes_[index].nodeId();
    }

    void flush()
    {
        if (surface_.blocks.isEmpty())
            return;
        parts_.append(QVariantMap{ { QStringLiteral("kind"), QStringLiteral("text") },
                                   { QStringLiteral("surface"), QVariant::fromValue(surface_) } });
        surface_ = {};
    }

    void begin(qsizetype index, int quote, int indent, qreal spacing = 8)
    {
        MarkupTextBlock block;
        block.key = key(index);
        block.format.setLeftMargin(quote * 14);
        block.format.setIndent(indent);
        block.format.setTopMargin(surface_.blocks.isEmpty() ? 0 : spacing);
        surface_.blocks.append(std::move(block));
    }

    void children(qsizetype index, const MarkupTextRun& style, int quote, int indent)
    {
        for (qsizetype child : children_[index])
            node(child, style, quote, indent);
    }

    void table(qsizetype index, const MarkupTextRun& style, int quote, int indent)
    {
        flush();
        const auto alignments = nodes_[index].table().columns();
        if (alignments.isEmpty())
            return;
        QVariantList rows;
        for (qsizetype row : children_[index]) {
            QVariantList cells;
            for (qsizetype column = 0; column < alignments.size(); ++column) {
                const auto rowChildren = children_[row];
                const qsizetype cell = column < rowChildren.size() ? rowChildren[column] : row;
                begin(cell, 0, 0);
                auto& block = surface_.blocks.last();
                if (alignments[column] == ColumnAlignment::COLUMN_ALIGNMENT_CENTER)
                    block.format.setAlignment(Qt::AlignHCenter);
                else if (alignments[column] == ColumnAlignment::COLUMN_ALIGNMENT_RIGHT)
                    block.format.setAlignment(Qt::AlignRight);
                auto cellStyle = style;
                if (nodes_[row].tableRowHeader())
                    cellStyle.format.setFontWeight(QFont::Bold);
                if (column < rowChildren.size())
                    children(cell, cellStyle, 0, 0);
                surface_.separator = column == 0 ? QStringLiteral("\n") : QStringLiteral("\t");
                cells.append(QVariant::fromValue(surface_));
                surface_ = {};
            }
            rows.append(QVariantMap{ { QStringLiteral("header"), nodes_[row].tableRowHeader() },
                                     { QStringLiteral("cells"), cells } });
        }
        parts_.append(QVariantMap{ { QStringLiteral("kind"), QStringLiteral("table") },
                                   { QStringLiteral("rows"), rows },
                                   { QStringLiteral("columns"), alignments.size() },
                                   { QStringLiteral("indent"), quote * 14 + indent * 40 } });
    }

    void list(qsizetype index, const MarkupTextRun& style, int quote, int indent)
    {
        QTextListFormat format;
        format.setIndent(indent + 1);
        format.setStyle(nodes_[index].list().hasStart() ? QTextListFormat::ListDecimal : QTextListFormat::ListDisc);
        format.setStart(nodes_[index].list().hasStart() ? nodes_[index].list().start() : 1);
        int ordinal = 0;
        for (qsizetype item : children_[index]) {
            begin(item, quote, 0, 2);
            surface_.blocks.last().list = format;
            surface_.blocks.last().list.setStart(format.start() + ordinal++);
            surface_.blocks.last().listKey = key(index);
            bool first = true;
            for (qsizetype child : children_[item]) {
                if (first && nodes_[child].hasContainer() &&
                    nodes_[child].container() == ContainerKind::CONTAINER_KIND_PARAGRAPH)
                    children(child, style, quote, indent + 1);
                else
                    node(child, style, quote, indent + 1);
                first = false;
            }
        }
    }

    void node(qsizetype index, MarkupTextRun style, int quote, int indent)
    {
        const auto& value = nodes_[index];
        if (value.hasText() || value.hasAnnotation()) {
            if (surface_.blocks.isEmpty())
                begin(index, quote, indent);
            style.key = key(index);
            if (value.hasText()) {
                style.text = value.text().value().text();
                switch (value.text().kind()) {
                    case TextKind::TEXT_KIND_INLINE_CODE:
                        style.code = true;
                        break;
                    case TextKind::TEXT_KIND_SOFT_BREAK:
                        style.text = QStringLiteral(" ");
                        break;
                    case TextKind::TEXT_KIND_HARD_BREAK:
                        style.text = QChar::LineSeparator;
                        break;
                    default:
                        break;
                }
            } else {
                style.text = value.annotation().label().text();
                style.annotation = true;
            }
            surface_.blocks.last().runs.append(std::move(style));
        } else if (value.hasList()) {
            list(index, style, quote, indent);
        } else if (value.hasTable()) {
            table(index, style, quote, indent);
        } else if (value.hasTaskChecked()) {
            if (!surface_.blocks.isEmpty())
                surface_.blocks.last().format.setMarker(value.taskChecked() ? QTextBlockFormat::MarkerType::Checked
                                                                            : QTextBlockFormat::MarkerType::Unchecked);
        } else if (value.hasRule()) {
            begin(index, quote, indent);
            surface_.blocks.last().format.setProperty(
              QTextFormat::BlockTrailingHorizontalRulerWidth,
              QVariant::fromValue(QTextLength(QTextLength::PercentageLength, 100)));
        } else {
            if (value.hasHeadingLevel()) {
                begin(index, quote, indent);
                surface_.blocks.last().format.setHeadingLevel(value.headingLevel());
                style.scale = 1.0 + (7 - value.headingLevel()) * 0.12;
                style.format.setFontWeight(QFont::Bold);
            } else if (value.hasCodeBlock()) {
                begin(index, quote, indent);
                style.code = true;
            } else if (value.hasLink()) {
                style.format.setAnchor(true);
                style.format.setAnchorHref(value.link().target());
                style.format.setToolTip(value.link().title());
                style.format.setFontUnderline(true);
            } else if (value.hasContainer()) {
                switch (value.container()) {
                    case ContainerKind::CONTAINER_KIND_PARAGRAPH:
                        begin(index, quote, indent);
                        break;
                    case ContainerKind::CONTAINER_KIND_BLOCK_QUOTE:
                        ++quote;
                        break;
                    case ContainerKind::CONTAINER_KIND_EMPHASIS:
                        style.format.setFontItalic(true);
                        break;
                    case ContainerKind::CONTAINER_KIND_STRONG:
                        style.format.setFontWeight(QFont::Bold);
                        break;
                    case ContainerKind::CONTAINER_KIND_STRIKETHROUGH:
                        style.format.setFontStrikeOut(true);
                        break;
                    default:
                        break;
                }
            }
            children(index, style, quote, indent);
        }
    }

    QString blockId_;
    QList<SemanticNode> nodes_;
    QList<QList<qsizetype>> children_;
    MarkupTextSurface surface_;
    QVariantList parts_;
};
}

QList<MarkupTextRun>
MarkupTextSurface::selectionRuns() const
{
    QList<MarkupTextRun> runs;
    bool first = true;
    for (const auto& block : blocks) {
        // Empty blocks also need an endpoint, including empty table cells.
        runs.append(
          { .key = block.key + QStringLiteral("/boundary"), .text = first ? QString() : QStringLiteral("\n") });
        runs.append(block.runs);
        first = false;
    }
    return runs;
}

QString
MarkupTextSurface::text() const
{
    QString result;
    for (const auto& run : selectionRuns())
        result += run.text;
    return result;
}

QVariantList
markupRenderParts(const ward::markup::v1::SemanticDocument& document)
{
    return Projector().project(document);
}

QVariantList
markupLiteralPart(const QString& key, const QString& text)
{
    MarkupTextSurface surface;
    surface.blocks.append({ .key = key, .runs = { { .key = key + QStringLiteral("/literal"), .text = text } } });
    return { QVariantMap{ { QStringLiteral("kind"), QStringLiteral("text") },
                          { QStringLiteral("surface"), QVariant::fromValue(surface) } } };
}

QList<MarkupTextSurface>
markupSurfaces(const QVariantList& parts)
{
    QList<MarkupTextSurface> result;
    for (const auto& partValue : parts) {
        const auto part = partValue.toMap();
        if (part.contains(QStringLiteral("surface"))) {
            result.append(part.value(QStringLiteral("surface")).value<MarkupTextSurface>());
        } else {
            for (const auto& row : part.value(QStringLiteral("rows")).toList()) {
                for (const auto& cell : row.toMap().value(QStringLiteral("cells")).toList())
                    result.append(cell.value<MarkupTextSurface>());
            }
        }
    }
    return result;
}
