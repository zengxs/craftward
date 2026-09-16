// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "syntaxhighlightingdocument.h"
#include "highlight.qpb.h"
#include "syntaxhighlightingwire_p.h"
#include "ward/coreffierror.h"
#include <ward_core.h>

#include <QProtobufSerializer>
#include <QThread>
#include <QTimer>
#include <limits>

namespace craftward::highlighting {
namespace {
struct Runtime
{
    QThread thread;
    Runtime()
    {
        thread.setObjectName(QStringLiteral("Syntax highlighting"));
        thread.start();
    }
    ~Runtime()
    {
        thread.quit();
        thread.wait();
    }
};
QThread*
highlightingThread()
{
    static Runtime runtime;
    return &runtime.thread;
}
struct DocumentDeleter
{
    void operator()(WardSyntaxDocument* document) const { ward_core_syntax_document_destroy(document); }
};
struct BufferDeleter
{
    void operator()(WardOwnedBuffer* buffer) const { ward_core_owned_buffer_destroy(buffer); }
};
QByteArray
configurationBytes(const QString& language, const QString& name, Theme theme)
{
    ward::highlighting::v1::DocumentConfiguration configuration;
    configuration.setLanguage(language);
    configuration.setFileName(name);
    configuration.setDarkTheme(theme == Theme::Dark);
    QProtobufSerializer serializer;
    return configuration.serialize(&serializer);
}
const std::uint8_t*
bytes(const QByteArray& value)
{
    return reinterpret_cast<const std::uint8_t*>(value.constData());
}
}

class SyntaxHighlightingDocumentWorker final : public QObject
{
    Q_OBJECT
  public:
    std::unique_ptr<WardSyntaxDocument, DocumentDeleter> document;
    quint64 epoch = 0, revision = 0, configuration = 0;
    bool paused = false, scheduled = false, awaiting = false;

    void reset(quint64 newEpoch, const QByteArray& source, const QByteArray& config)
    {
        epoch = newEpoch;
        revision = configuration = 0;
        awaiting = false;
        QString error;
        document.reset(SyntaxHighlightingEngine::shared()->createDocument(source, config, error));
        if (!document) {
            fail(error);
            return;
        }
        schedule();
    }
    void edit(const QByteArray& edit)
    {
        ++revision;
        if (!document)
            return;
        WardError* error = nullptr;
        if (!ward_core_syntax_document_edit(document.get(), bytes(edit), edit.size(), &error)) {
            fail(ward::coreffi::takeErrorMessage(error));
            return;
        }
        schedule();
    }
    void configure(const QByteArray& config)
    {
        ++configuration;
        if (!document)
            return;
        WardError* error = nullptr;
        if (!ward_core_syntax_document_configure(document.get(), bytes(config), config.size(), &error)) {
            fail(ward::coreffi::takeErrorMessage(error));
            return;
        }
        schedule();
    }
    void acknowledge(quint64 resultEpoch, quint64 sequence, bool applied)
    {
        if (resultEpoch != epoch || !document)
            return;
        ward_core_syntax_document_acknowledge(document.get(), sequence, applied);
        awaiting = false;
        schedule();
    }
    void schedule()
    {
        if (paused || awaiting || scheduled || !document)
            return;
        scheduled = true;
        QTimer::singleShot(0, this, [this] {
            scheduled = false;
            if (paused || awaiting || !document)
                return;
            step();
        });
    }
  signals:
    void ready(const craftward::highlighting::DocumentStyles& styles);
    void failed(quint64 epoch, quint64 revision, quint64 configuration, const QString& message);

  private:
    void fail(QString message)
    {
        document.reset();
        awaiting = false;
        if (message.isEmpty())
            message = QStringLiteral("The syntax-highlighting document could not be updated.");
        emit failed(epoch, revision, configuration, message);
    }
    void step()
    {
        WardError* rawError = nullptr;
        std::unique_ptr<WardOwnedBuffer, BufferDeleter> buffer(
          ward_core_syntax_document_step(document.get(), 32, 16384, &rawError));
        if (!buffer) {
            if (rawError)
                fail(ward::coreffi::takeErrorMessage(rawError));
            return;
        }
        const auto size = ward_core_owned_buffer_size(buffer.get());
        if (size > size_t(std::numeric_limits<qsizetype>::max())) {
            fail(QStringLiteral("The highlighting batch is too large."));
            return;
        }
        const QByteArrayView payload(reinterpret_cast<const char*>(ward_core_owned_buffer_data(buffer.get())),
                                     qsizetype(size));
        ward::highlighting::v1::DocumentStyles decoded;
        QProtobufSerializer serializer;
        if (!decoded.deserialize(&serializer, payload) || decoded.start() > decoded.end() ||
            decoded.end() > quint64(std::numeric_limits<qsizetype>::max())) {
            fail(QStringLiteral("Invalid highlighting batch."));
            return;
        }
        DocumentStyles styles{ .epoch = epoch,
                               .revision = decoded.revision(),
                               .configuration = decoded.configuration(),
                               .sequence = decoded.sequence(),
                               .start = qsizetype(decoded.start()),
                               .end = qsizetype(decoded.end()),
                               .result = Result{ .syntaxName = decoded.syntaxName(),
                                                 .languageRecognized = decoded.languageRecognized() },
                               .complete = decoded.complete() };
        quint64 previous = decoded.start();
        for (const auto& span : decoded.spans()) {
            if (span.utf8Start() < previous || span.utf8End() < span.utf8Start() || span.utf8End() > decoded.end() ||
                !span.hasStyle() || !span.style().hasForeground() || !span.style().hasBackground()) {
                fail(QStringLiteral("Invalid highlighting span."));
                return;
            }
            previous = span.utf8End();
            styles.result.spans.append(
              Span{ qsizetype(span.utf8Start()), qsizetype(span.utf8End()), styleFromWire(span.style()) });
        }
        awaiting = true;
        emit ready(styles);
    }
};

SyntaxHighlightingDocument::SyntaxHighlightingDocument(QObject* parent)
  : QObject(parent)
  , worker(new SyntaxHighlightingDocumentWorker)
{
    worker->moveToThread(highlightingThread());
    connect(worker, &SyntaxHighlightingDocumentWorker::ready, this, [this](const DocumentStyles& styles) {
        if (styles.epoch != epoch || styles.revision != revision || styles.configuration != configuration || paused) {
            acknowledge(styles, false);
            return;
        }
        emit stylesReady(styles);
    });
    connect(worker,
            &SyntaxHighlightingDocumentWorker::failed,
            this,
            [this](quint64 resultEpoch, quint64 resultRevision, quint64 resultConfiguration, const QString& message) {
                if (resultEpoch != epoch)
                    return;
                if (resultRevision != revision || resultConfiguration != configuration)
                    emit resyncRequired();
                else
                    emit failed(message);
            });
}
SyntaxHighlightingDocument::~SyntaxHighlightingDocument()
{
    worker->deleteLater();
}

void
SyntaxHighlightingDocument::reset(QByteArray source, QString language, QString fileName, Theme theme)
{
    ++epoch;
    revision = configuration = 0;
    const auto config = configurationBytes(language, fileName, theme);
    QMetaObject::invokeMethod(worker, [worker = worker, epoch = epoch, source = std::move(source), config] {
        worker->reset(epoch, source, config);
    });
}
void
SyntaxHighlightingDocument::edit(qsizetype start, qsizetype deleted, QByteArray inserted)
{
    ward::highlighting::v1::DocumentEdit edit;
    edit.setFromRevision(revision++);
    edit.setStart(start);
    edit.setDeleted(deleted);
    edit.setInserted(inserted);
    QProtobufSerializer serializer;
    const auto payload = edit.serialize(&serializer);
    QMetaObject::invokeMethod(worker, [worker = worker, payload] { worker->edit(payload); });
}
void
SyntaxHighlightingDocument::configure(QString language, QString fileName, Theme theme)
{
    ++configuration;
    const auto config = configurationBytes(language, fileName, theme);
    QMetaObject::invokeMethod(worker, [worker = worker, config] { worker->configure(config); });
}
void
SyntaxHighlightingDocument::setPaused(bool value)
{
    paused = value;
    QMetaObject::invokeMethod(worker, [worker = worker, value] {
        worker->paused = value;
        worker->schedule();
    });
}
void
SyntaxHighlightingDocument::acknowledge(const DocumentStyles& styles, bool applied)
{
    QMetaObject::invokeMethod(worker, [worker = worker, epoch = styles.epoch, sequence = styles.sequence, applied] {
        worker->acknowledge(epoch, sequence, applied);
    });
}
}

#include "syntaxhighlightingdocument.moc"
