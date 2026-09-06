// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

// Throwaway experiment: semantic spans -> native TextEdit document -> QML controls.
#include "MessageSelection.h"

#include <QAbstractTextDocumentLayout>
#include <QClipboard>
#include <QDir>
#include <QFile>
#include <QFontDatabase>
#include <QFontMetricsF>
#include <QGuiApplication>
#include <QImage>
#include <QJsonDocument>
#include <QPointer>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QQuickTextDocument>
#include <QQuickWindow>
#include <QTextBlock>
#include <QTextCursor>
#include <QTextDocument>
#include <QTextLayout>
#include <QTimer>
#include <QVariantList>

#include <algorithm>
#include <cmath>

class InlineDocument : public QObject
{
    Q_OBJECT
    Q_PROPERTY(QQuickTextDocument* document READ document WRITE setDocument NOTIFY changed)
    Q_PROPERTY(QVariantList nodes MEMBER nodes_ WRITE setNodes NOTIFY changed)
    Q_PROPERTY(bool dark MEMBER dark_ WRITE setDark NOTIFY changed)
    Q_PROPERTY(int revision READ revision NOTIFY changed)

  public:
    using QObject::QObject;

    int revision() const { return revision_; }
    QQuickTextDocument* document() const { return wrapper_; }

    void setDocument(QQuickTextDocument* wrapper)
    {
        if (wrapper_ == wrapper)
            return;
        wrapper_ = wrapper;
        rebuild();
    }

    void setNodes(const QVariantList& nodes)
    {
        if (nodes_ == nodes)
            return;
        nodes_ = nodes;
        rebuild();
    }

    void setDark(bool dark)
    {
        if (dark_ == dark)
            return;
        dark_ = dark;
        if (native()) {
            QTextCursor transaction(native());
            transaction.beginEditBlock();
            for (const Span& span : spans_) {
                QTextCursor cursor(native());
                cursor.setPosition(span.start);
                cursor.setPosition(span.start + span.length, QTextCursor::KeepAnchor);
                cursor.setCharFormat(format(span.node));
            }
            transaction.endEditBlock();
        }
        mutation_ = "theme";
        ++revision_;
        emit changed();
    }

    Q_INVOKABLE QVariantMap nodeState(const QString& id) const
    {
        for (const Span& span : spans_) {
            if (span.node.value("id").toString() != id)
                continue;
            QVariantMap result = span.node;
            result.insert("start", span.start);
            result.insert("length", span.length);
            if (span.node.value("kind") == "control") {
                QTextCursor cursor(native());
                cursor.setPosition(span.start);
                cursor.setPosition(span.start + span.length, QTextCursor::KeepAnchor);
                const QTextImageFormat image = cursor.charFormat().toImageFormat();
                result.insert("width", image.width());
                result.insert("height", image.height());
            }
            return result;
        }
        return {};
    }

    Q_INVOKABLE QRectF controlRect(const QString& id) const
    {
        if (!native())
            return {};
        const QVariantMap state = nodeState(id);
        const int start = state.value("start").toInt();
        const QTextBlock block = native()->findBlock(start);
        const QRectF blockRect = native()->documentLayout()->blockBoundingRect(block);
        const int relative = start - block.position();
        const QTextLine line = block.layout()->lineForTextPosition(relative);
        if (!line.isValid())
            return {};
        const qreal left = std::min(line.cursorToX(relative), line.cursorToX(relative + 1));
        const qreal height = state.value("height").toReal();
        return { blockRect.x() + left,
                 blockRect.y() + line.y() + line.ascent() - height + QFontMetricsF(native()->defaultFont()).descent(),
                 state.value("width").toReal(),
                 height };
    }

    Q_INVOKABLE qreal baselineAt(int position) const
    {
        if (!native())
            return 0;
        const QTextBlock block = native()->findBlock(position);
        const QRectF box = native()->documentLayout()->blockBoundingRect(block);
        const QTextLine line = block.layout()->lineForTextPosition(position - block.position());
        return line.isValid() ? box.y() + line.y() + line.ascent() : 0;
    }

    Q_INVOKABLE void toggleControl(const QString& id)
    {
        if (!native())
            return;
        for (Span& span : spans_) {
            if (span.node.value("id").toString() != id)
                continue;
            const bool expanded = !span.node.value("expanded").toBool();
            span.node.insert("expanded", expanded);
            span.node.insert("text", expanded ? "Ready to review" : "Review");
            QTextCursor cursor(native());
            cursor.setPosition(span.start);
            cursor.setPosition(span.start + span.length, QTextCursor::KeepAnchor);
            cursor.setCharFormat(format(span.node));
            mutation_ = "resize control " + id;
            ++revision_;
            emit changed();
            return;
        }
    }

    Q_INVOKABLE void refreshControlMetrics()
    {
        if (!native())
            return;
        for (const Span& span : spans_) {
            if (span.node.value("kind") != "control")
                continue;
            QTextCursor cursor(native());
            cursor.setPosition(span.start);
            cursor.setPosition(span.start + span.length, QTextCursor::KeepAnchor);
            cursor.setCharFormat(format(span.node));
        }
        emit changed();
    }

    Q_INVOKABLE void appendText(const QString& text)
    {
        if (!native() || spans_.isEmpty() || spans_.last().node.value("kind") != "text")
            return;
        Span& tail = spans_.last();
        QTextCursor cursor(native());
        cursor.movePosition(QTextCursor::End);
        cursor.insertText(text, format(tail.node));
        tail.length += text.size();
        tail.node.insert("text", tail.node.value("text").toString() + text);
        mutation_ = "append to " + tail.node.value("id").toString();
        ++revision_;
        emit changed();
    }

    Q_INVOKABLE QString selectionText(int start, int end) const
    {
        QString result;
        if (!native())
            return result;
        for (const Span& span : spans_) {
            const int first = std::max(start, span.start);
            const int last = std::min(end, span.start + span.length);
            if (first >= last)
                continue;
            if (span.node.value("kind") == "control") {
                result += "[" + span.node.value("text").toString() + "]";
            } else {
                QTextCursor cursor(native());
                cursor.setPosition(first);
                cursor.setPosition(last, QTextCursor::KeepAnchor);
                result += cursor.selectedText();
            }
        }
        return result;
    }

    Q_INVOKABLE QString copySelection(int start, int end) const
    {
        const QString text = selectionText(start, end);
        if (!text.isEmpty())
            QGuiApplication::clipboard()->setText(text);
        return text;
    }

    Q_INVOKABLE QVariantMap endpointAt(int position) const
    {
        for (const Span& span : spans_) {
            if (position < span.start + span.length || &span == &spans_.last())
                return { { "nodeId", span.node.value("id") },
                         { "offset", std::clamp(position - span.start, 0, span.length) } };
        }
        return {};
    }

    Q_INVOKABLE QVariantMap wordAt(int position) const
    {
        if (!native())
            return {};
        QTextCursor cursor(native());
        cursor.setPosition(std::clamp(position, 0, native()->characterCount() - 1));
        cursor.select(QTextCursor::WordUnderCursor);
        return { { "start", endpointAt(cursor.selectionStart()) }, { "end", endpointAt(cursor.selectionEnd()) } };
    }

    Q_INVOKABLE QVariantMap linkFragmentAt(const QString& target, qreal x, qreal y) const
    {
        if (!native() || target.isEmpty())
            return {};
        const int position = native()->documentLayout()->hitTest({ x, y }, Qt::FuzzyHit);
        for (const Span& span : spans_) {
            if (span.length == 0 || span.node.value("target") != target || position < span.start ||
                position > span.start + span.length)
                continue;
            const int cursor = std::clamp(position, span.start, span.start + span.length - 1);
            const QTextBlock block = native()->findBlock(cursor);
            const QRectF box = native()->documentLayout()->blockBoundingRect(block);
            const QTextLayout* layout = block.layout();
            for (int i = 0; i < layout->lineCount(); ++i) {
                const QTextLine line = layout->lineAt(i);
                if (y < box.y() + line.y() || y >= box.y() + line.y() + line.height())
                    continue;
                const int first = std::max(span.start - block.position(), line.textStart());
                const int last =
                  std::min(span.start + span.length - block.position(), line.textStart() + line.textLength());
                if (first >= last)
                    continue;
                const qreal firstX = line.cursorToX(first);
                const qreal lastX = line.cursorToX(last);
                const QRectF rect(box.x() + std::min(firstX, lastX),
                                  box.y() + line.y(),
                                  std::max(qreal(1), std::abs(lastX - firstX)),
                                  line.height());
                const QString hint = span.node.value("hint").toString();
                return { { "nodeId", span.node.value("id") },
                         { "target", target },
                         { "rect", rect },
                         { "hint", hint.isEmpty() ? target : hint } };
            }
        }
        return {};
    }

    Q_INVOKABLE QVariantMap snapshot() const
    {
        QVariantList spans;
        for (const Span& span : spans_)
            spans.append(nodeState(span.node.value("id").toString()));
        return { { "revision", revision_ },
                 { "documentBuilds", builds_ },
                 { "lastMutation", mutation_ },
                 { "spans", spans },
                 { "copyText", selectionText(0, native() ? native()->characterCount() - 1 : 0) } };
    }

  signals:
    void changed();

  private:
    struct Span
    {
        QVariantMap node;
        int start;
        int length;
    };

    QTextDocument* native() const { return wrapper_ ? wrapper_->textDocument() : nullptr; }

    QTextCharFormat format(const QVariantMap& node) const
    {
        const QString kind = node.value("kind").toString();
        QTextCharFormat format;
        format.setProperty(QTextFormat::UserProperty + 1, node.value("id"));
        format.setProperty(QTextFormat::UserProperty + 2, kind);
        if (kind == "control") {
            // An image object reserves a native inline box; its QML control paints it.
            QTextImageFormat image;
            image.setName("inline-prototype:transparent");
            image.setWidth(QFontMetricsF(native()->defaultFont()).horizontalAdvance(node.value("text").toString()) +
                           28);
            image.setFont(native()->defaultFont());
            image.setHeight(QFontMetricsF(native()->defaultFont()).height());
            image.setVerticalAlignment(QTextCharFormat::AlignBaseline);
            image.setProperty(QTextFormat::ImageAltText, node.value("text"));
            image.setProperty(QTextFormat::UserProperty + 1, node.value("id"));
            return image;
        }
        if (kind == "code") {
            format.setFontFamilies({ QFontDatabase::systemFont(QFontDatabase::FixedFont).family() });
            format.setBackground(QColor(dark_ ? "#30333a" : "#edf0f5"));
        }
        if (kind == "link" || kind == "annotation") {
            format.setAnchor(true);
            format.setAnchorHref(node.value("target").toString());
            format.setForeground(QColor(dark_ ? "#93c5fd" : "#1d4ed8"));
            format.setFontUnderline(kind == "link");
        }
        if (kind == "annotation") {
            format.setFontWeight(QFont::DemiBold);
            format.setBackground(QColor(dark_ ? "#263b51" : "#e0edff"));
        }
        return format;
    }

    void rebuild()
    {
        if (!native() || nodes_.isEmpty())
            return;
        QTextDocument* document = native();
        document->setUndoRedoEnabled(false);
        document->setDocumentMargin(0);
        QImage transparent(1, 1, QImage::Format_ARGB32_Premultiplied);
        transparent.fill(Qt::transparent);
        document->addResource(QTextDocument::ImageResource, QUrl("inline-prototype:transparent"), transparent);
        QTextCursor cursor(document);
        cursor.beginEditBlock();
        cursor.select(QTextCursor::Document);
        cursor.removeSelectedText();
        spans_.clear();
        for (const QVariant& value : nodes_) {
            const QVariantMap node = value.toMap();
            const int start = cursor.position();
            if (node.value("kind") == "control")
                cursor.insertText(QString(QChar::ObjectReplacementCharacter), format(node));
            else
                cursor.insertText(node.value("text").toString(), format(node));
            spans_.append({ node, start, cursor.position() - start });
        }
        cursor.endEditBlock();
        mutation_ = "initial semantic fixture";
        ++builds_;
        ++revision_;
        emit changed();
    }

    QPointer<QQuickTextDocument> wrapper_;
    QVariantList nodes_;
    QList<Span> spans_;
    bool dark_ = false;
    int revision_ = 0;
    int builds_ = 0;
    QString mutation_;
};

class PrototypeCapture : public QObject
{
    Q_OBJECT
  public:
    using QObject::QObject;

    Q_INVOKABLE void report(const QVariantMap& state)
    {
        bool aligned = true;
        int count = 0;
        for (const QVariant& block : state.value("blocks").toList()) {
            for (const QVariant& control : block.toMap().value("controls").toList()) {
                ++count;
                const qreal error = control.toMap().value("baselineError").toReal();
                const QVariantMap rect = control.toMap().value("rect").toMap();
                const qreal top = rect.value("y").toReal();
                const qreal bottom = top + rect.value("height").toReal();
                aligned = aligned && std::isfinite(error) && std::abs(error) <= 1 && top >= -1 &&
                          bottom <= block.toMap().value("height").toReal() + 1;
            }
        }
        qInfo().noquote() << "INLINE_PROTOTYPE_PROBE"
                          << QJsonDocument::fromVariant(state).toJson(QJsonDocument::Compact);
        QCoreApplication::exit(aligned && count == 1 ? 0 : 2);
    }

    Q_INVOKABLE void save(QQuickWindow* window, const QVariantMap& state)
    {
        const QString directory = qEnvironmentVariable("CRAFTWARD_INLINE_PROTOTYPE_OUTPUT_DIR");
        if (directory.isEmpty() || !window)
            return;
        QDir().mkpath(directory);
        const QString prefix = directory + "/capture-" + QString::number(++sequence_);
        QTimer::singleShot(0, window, [window, state, prefix] {
            window->grabWindow().save(prefix + ".png");
            QFile file(prefix + ".json");
            if (file.open(QIODevice::WriteOnly))
                file.write(QJsonDocument::fromVariant(state).toJson());
            qInfo().noquote() << "Prototype capture:" << prefix;
        });
    }

    Q_INVOKABLE void reportSelection(const QVariantMap& state)
    {
        const QVariantList checks = state.value("checks").toList();
        const bool passed = !checks.isEmpty() && std::all_of(checks.begin(), checks.end(), [](const QVariant& check) {
            return check.toMap().value("passed").toBool();
        });
        qInfo().noquote() << "SELECTION_PROTOTYPE_PROBE"
                          << QJsonDocument::fromVariant(state).toJson(QJsonDocument::Compact);
        QCoreApplication::exit(passed ? 0 : 2);
    }

  private:
    int sequence_ = 0;
};

int
main(int argc, char** argv)
{
    QGuiApplication app(argc, argv);
    QGuiApplication::setApplicationName("Craftward Inline Prototype");
    QQuickStyle::setStyle("Basic");
    qmlRegisterType<InlineDocument>("Craftward.InlinePrototype", 1, 0, "InlineDocument");
    qmlRegisterType<MessageSelection>("Craftward.InlinePrototype", 1, 0, "MessageSelection");
    PrototypeCapture capture;
    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty("prototypeCapture", &capture);
    engine.rootContext()->setContextProperty("probeMode", app.arguments().contains("--probe"));
    engine.rootContext()->setContextProperty("selectionProbeMode", app.arguments().contains("--selection-probe"));
    engine.rootContext()->setContextProperty("tooltipProbeMode", app.arguments().contains("--tooltip-probe"));
    engine.rootContext()->setContextProperty("probeFontSize", qEnvironmentVariableIntValue("INLINE_PROBE_FONT"));
    engine.rootContext()->setContextProperty("probeWidth", qEnvironmentVariableIntValue("INLINE_PROBE_WIDTH"));
    engine.rootContext()->setContextProperty("probeExpanded",
                                             qEnvironmentVariableIntValue("INLINE_PROBE_EXPANDED") == 1);
    const bool selectionMode = app.arguments().contains("--selection") ||
                               app.arguments().contains("--selection-probe") ||
                               app.arguments().contains("--tooltip-probe");
    engine.load(QUrl(selectionMode ? "qrc:/SelectionMain.qml" : "qrc:/Main.qml"));
    if (engine.rootObjects().isEmpty())
        return 1;
    return app.exec();
}

#include "main.moc"
