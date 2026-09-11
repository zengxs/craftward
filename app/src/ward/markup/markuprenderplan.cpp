// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "markuprenderplan.h"

#include <QDir>

#include <algorithm>
#include <array>
#include <utility>

namespace {
using namespace ward::markup::v1;
using ContainerKind = ContainerKindGadget::ContainerKind;
using TextKind = TextKindGadget::TextKind;
using ColumnAlignment = ColumnAlignmentGadget::ColumnAlignment;

struct CommentFileLocation
{
    QString path;
    QString displayPath;
};

CommentFileLocation
commentFileLocation(const QString& file, const QString& baseDirectory)
{
    if (baseDirectory.isEmpty() || !QDir::isAbsolutePath(baseDirectory))
        return { file, file };

    const QString basePrefix =
      baseDirectory.endsWith(QLatin1Char('/')) ? baseDirectory : baseDirectory + QLatin1Char('/');
    if (QDir::isAbsolutePath(file))
        return { file, file.startsWith(basePrefix) ? file.mid(basePrefix.size()) : file };

    QString relative = file;
    const QString workspacePrefix = QDir(baseDirectory).dirName() + QLatin1Char('/');
    if (relative.startsWith(workspacePrefix))
        relative.remove(0, workspacePrefix.size());
    // Concatenate without lexical cleanup: link/.. must be resolved by the filesystem.
    // A leading ./ explicitly addresses a relative directory with the workspace's name.
    return { basePrefix + relative, relative };
}

class Projector
{
  public:
    QVariantList project(const SemanticDocument& document,
                         const QHash<QString, MarkupListContext>& listContexts,
                         const QString& baseDirectory)
    {
        baseDirectory_ = baseDirectory;
        for (const auto& block : document.blocks()) {
            blockId_ = block.blockId();
            const auto context = listContexts.constFind(blockId_);
            listContext_ = context != listContexts.cend() ? context.value() : markupListContext(block);
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
        surface_.listNumberDigits = std::max(surface_.listNumberDigits, listContext_.numberDigits);
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
                                   { QStringLiteral("quoteIndent"), quote * 14 },
                                   { QStringLiteral("listDepth"), indent } });
    }

    void list(qsizetype index, const MarkupTextRun& style, int quote, int indent)
    {
        QTextListFormat format;
        format.setIndent(indent + 1);
        constexpr std::array bullets{ QTextListFormat::ListDisc,
                                      QTextListFormat::ListCircle,
                                      QTextListFormat::ListSquare };
        format.setStyle(nodes_[index].list().hasStart() ? QTextListFormat::ListDecimal
                                                        : bullets[indent % bullets.size()]);
        format.setStart(nodes_[index].list().hasStart() ? nodes_[index].list().start() : 1);
        const qreal spacing = index == 0 ? listContext_.rootItemSpacing : markupListItemSpacing(nodes_, index);
        int ordinal = 0;
        for (qsizetype item : children_[index]) {
            // A first item after a table starts a separate block, not an item boundary.
            if (ordinal > 0 && surface_.blocks.isEmpty() && !parts_.isEmpty()) {
                auto part = parts_.last().toMap();
                part.insert(QStringLiteral("spacingAfter"), spacing);
                parts_.last() = part;
            }
            begin(item, quote, 0, spacing);
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

    void codeComment(qsizetype index)
    {
        flush();
        auto surrounding = std::exchange(parts_, {});
        children(index, {}, 0, 0);
        flush();
        const auto body = std::exchange(parts_, std::move(surrounding));
        const auto comment = nodes_[index].codeComment();
        const QString prefix = key(index);
        const auto text = [&](const QString& field, const QString& value, const QString& separator, bool link = false) {
            MarkupTextSurface surface;
            surface.separator = separator;
            MarkupTextRun run{ .key = prefix + QLatin1Char('/') + field, .text = value };
            if (link) {
                run.format.setAnchor(true);
                run.format.setAnchorHref(QStringLiteral("code-comment:location"));
                run.format.setFontUnderline(true);
            }
            surface.blocks.append({ .key = run.key, .runs = { run } });
            return QVariant::fromValue(surface);
        };
        const QString priority = comment.hasPriority() ? QStringLiteral("P%1").arg(comment.priority()) : QString();
        QString title = comment.title().text();
        if (!priority.isEmpty() && title.startsWith(QLatin1Char('[') + priority + QStringLiteral("] ")) &&
            !title.mid(priority.size() + 3).trimmed().isEmpty())
            title.remove(0, priority.size() + 3);
        const auto file = commentFileLocation(comment.file().text(), baseDirectory_);
        QString location = file.displayPath;
        if (comment.hasStart()) {
            location += QLatin1Char(':') + QString::number(comment.start());
            if (comment.hasEnd() && comment.end() != comment.start())
                location += QChar(0x2013) + QString::number(comment.end());
        }
        QVariantMap part{
            { QStringLiteral("kind"), QStringLiteral("codeComment") },
            { QStringLiteral("priority"), comment.hasPriority() ? int(comment.priority()) : -1 },
            { QStringLiteral("title"),
              text(QStringLiteral("title"), title, priority.isEmpty() ? QStringLiteral("\n\n") : QStringLiteral(" ")) },
            { QStringLiteral("location"), text(QStringLiteral("location"), location, QStringLiteral("\n"), true) },
            { QStringLiteral("file"), file.path },
            { QStringLiteral("start"), comment.hasStart() ? int(comment.start()) : 0 },
            { QStringLiteral("end"),
              comment.hasEnd() ? int(comment.end()) : (comment.hasStart() ? int(comment.start()) : 0) },
            { QStringLiteral("body"), body },
            { QStringLiteral("spacingAfter"), 12 }
        };
        if (!priority.isEmpty())
            part.insert(QStringLiteral("badge"), text(QStringLiteral("priority"), priority, QStringLiteral("\n\n")));
        parts_.append(part);
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
        } else if (value.hasCodeComment()) {
            codeComment(index);
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
    QString baseDirectory_;
    MarkupListContext listContext_;
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
markupRenderParts(const ward::markup::v1::SemanticDocument& document,
                  const QHash<QString, MarkupListContext>& listContexts,
                  const QString& baseDirectory)
{
    return Projector().project(document, listContexts, baseDirectory);
}

MarkupListContext
markupListContext(const ward::markup::v1::SemanticBlock& block)
{
    return { .numberDigits = markupListNumberDigits(block), .rootItemSpacing = markupListItemSpacing(block.nodes()) };
}

int
markupListNumberDigits(const ward::markup::v1::SemanticBlock& block)
{
    const auto nodes = block.nodes();
    QList<int> itemCounts(nodes.size(), 0);
    for (const auto& node : nodes)
        if (node.hasParentIndex() && node.parentIndex() < nodes.size() && node.hasContainer() &&
            node.container() == ContainerKind::CONTAINER_KIND_LIST_ITEM)
            ++itemCounts[node.parentIndex()];
    int digits = 0;
    for (qsizetype i = 0; i < nodes.size(); ++i)
        if (nodes[i].hasList() && nodes[i].list().hasStart()) {
            const quint64 last = nodes[i].list().start() + std::max(0, itemCounts[i] - 1);
            digits = std::max(digits, int(QString::number(last).size()));
        }
    return digits;
}

qreal
markupListItemSpacing(const QList<ward::markup::v1::SemanticNode>& nodes, qsizetype listIndex)
{
    // Tight-list text is inline; only direct item paragraphs make this list loose.
    const bool loose = std::any_of(nodes.cbegin(), nodes.cend(), [&](const SemanticNode& node) {
        if (!node.hasContainer() || node.container() != ContainerKind::CONTAINER_KIND_PARAGRAPH ||
            !node.hasParentIndex() || node.parentIndex() >= nodes.size())
            return false;
        const auto& parent = nodes[node.parentIndex()];
        return parent.hasParentIndex() && parent.parentIndex() == listIndex && parent.hasContainer() &&
               parent.container() == ContainerKind::CONTAINER_KIND_LIST_ITEM;
    });
    return loose ? 10 : 0;
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
        } else if (part.value(QStringLiteral("kind")) == QStringLiteral("codeComment")) {
            for (const auto& field : { QStringLiteral("badge"), QStringLiteral("title"), QStringLiteral("location") })
                if (part.contains(field))
                    result.append(part.value(field).value<MarkupTextSurface>());
            auto body = markupSurfaces(part.value(QStringLiteral("body")).toList());
            if (!body.isEmpty())
                body.first().separator = QStringLiteral("\n\n");
            result.append(body);
        } else {
            for (const auto& row : part.value(QStringLiteral("rows")).toList()) {
                for (const auto& cell : row.toMap().value(QStringLiteral("cells")).toList())
                    result.append(cell.value<MarkupTextSurface>());
            }
        }
    }
    return result;
}
