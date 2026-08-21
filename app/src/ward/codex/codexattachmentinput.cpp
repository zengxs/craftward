// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexattachmentinput.h"

#include "ward/coreffierror.h"

#include <ward_core.h>

#include <QClipboard>
#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QImage>
#include <QMimeData>
#include <QMimeDatabase>
#include <QPixmap>
#include <QSaveFile>
#include <QSet>
#include <QStandardPaths>
#include <QUuid>
#include <QVariant>
#include <QtTranslation>

namespace {
void
clearError(QString* errorMessage)
{
    if (errorMessage != nullptr)
        errorMessage->clear();
}

void
writeError(QString* errorMessage, const QString& message)
{
    if (errorMessage != nullptr)
        *errorMessage = message;
}

CodexAttachmentKind
attachmentKind(const QString& mimeName)
{
    if (mimeName.startsWith(QLatin1StringView("image/")))
        return CodexAttachmentKind::LocalImage;
    if (mimeName.startsWith(QLatin1StringView("audio/")))
        return CodexAttachmentKind::LocalAudio;
    return CodexAttachmentKind::Mention;
}

QString
normalizedPath(const QFileInfo& fileInfo)
{
    const QString canonicalPath = fileInfo.canonicalFilePath();
    return QDir::cleanPath(canonicalPath.isEmpty() ? fileInfo.absoluteFilePath() : canonicalPath);
}

QImage
clipboardImage(const QMimeData& mimeData)
{
    const QVariant imageData = mimeData.imageData();
    QImage image = qvariant_cast<QImage>(imageData);
    if (image.isNull() && imageData.canConvert<QPixmap>())
        image = qvariant_cast<QPixmap>(imageData).toImage();
    return image;
}
}

std::optional<QList<CodexAttachmentDescriptor>>
CodexAttachmentInput::describe(const QList<QUrl>& attachments, bool managed, QString* errorMessage)
{
    clearError(errorMessage);
    QList<CodexAttachmentDescriptor> described;
    described.reserve(attachments.size());
    QSet<QString> seenPaths;
    QMimeDatabase mimeDatabase;

    for (const QUrl& attachment : attachments) {
        if (!attachment.isLocalFile()) {
            writeError(errorMessage,
                       /*% "Only local files can be attached." */ qtTrId("craftward.codex.attachment.local_only"));
            return std::nullopt;
        }
        const QFileInfo fileInfo(attachment.toLocalFile());
        if (!fileInfo.exists() || !fileInfo.isFile()) {
            writeError(
              errorMessage,
              /*% "An attached file is no longer available: %1" */ qtTrId("craftward.codex.attachment.error.missing")
                .arg(attachment.fileName()));
            return std::nullopt;
        }
        if (!fileInfo.isReadable()) {
            writeError(
              errorMessage,
              /*% "An attached file is not readable: %1" */ qtTrId("craftward.codex.attachment.error.unreadable")
                .arg(fileInfo.fileName()));
            return std::nullopt;
        }

        const QString path = normalizedPath(fileInfo);
        if (seenPaths.contains(path))
            continue;
        seenPaths.insert(path);
        const QString mimeName = mimeDatabase.mimeTypeForFile(fileInfo, QMimeDatabase::MatchContent).name();
        described.append(
          { QUrl::fromLocalFile(path), fileInfo.fileName(), mimeName, attachmentKind(mimeName), managed });
    }
    return described;
}

std::optional<QList<CodexTurnAttachment>>
CodexAttachmentInput::prepare(const QList<QUrl>& attachments, QString* errorMessage)
{
    const std::optional<QList<CodexAttachmentDescriptor>> described = describe(attachments, false, errorMessage);
    if (!described.has_value())
        return std::nullopt;

    QList<CodexTurnAttachment> prepared;
    prepared.reserve(described->size());
    for (const CodexAttachmentDescriptor& attachment : *described) {
        prepared.append({ attachment.kind, attachment.name.toUtf8(), attachment.url.toLocalFile().toUtf8() });
    }
    return prepared;
}

QUrl
CodexAttachmentInput::storeClipboardImage(const QImage& image, const QString& dataRoot, QString* errorMessage)
{
    clearError(errorMessage);
    if (dataRoot.isEmpty()) {
        writeError(errorMessage,
                   /*% "The application data directory is unavailable." */ qtTrId(
                     "craftward.attachment_storage.error.data_directory_unavailable"));
        return {};
    }

    QDir dataDirectory(dataRoot);
    const QString incomingDirectory = QStringLiteral("attachments/.incoming");
    if (!dataDirectory.mkpath(incomingDirectory)) {
        writeError(errorMessage,
                   /*% "The clipboard image could not be saved." */ qtTrId("craftward.clipboard_image.error.save"));
        return {};
    }

    const QString stagingPath =
      dataDirectory.filePath(incomingDirectory + QStringLiteral("/clipboard-") +
                             QUuid::createUuid().toString(QUuid::WithoutBraces) + QStringLiteral(".png"));
    QSaveFile output(stagingPath);
    const QImage normalizedImage = image.convertToFormat(QImage::Format_RGBA8888);
    if (normalizedImage.isNull() || !output.open(QIODevice::WriteOnly) || !normalizedImage.save(&output, "PNG") ||
        !output.commit()) {
        writeError(errorMessage,
                   /*% "The clipboard image could not be saved." */ qtTrId("craftward.clipboard_image.error.save"));
        return {};
    }

    WardBlake3Digest digest{};
    WardError* rawError = nullptr;
    const QByteArray encodedStagingPath = stagingPath.toUtf8();
    if (!ward_core_blake3_hash_file(encodedStagingPath.constData(), &digest, &rawError)) {
        QString message = ward::coreffi::takeErrorMessage(rawError);
        if (message.isEmpty())
            message =
              /*% "The clipboard image could not be indexed." */ qtTrId("craftward.clipboard_image.error.index");
        QFile::remove(stagingPath);
        writeError(errorMessage, message);
        return {};
    }

    const QByteArray digestBytes(reinterpret_cast<const char*>(digest.bytes), sizeof(digest.bytes));
    const QString digestHex = QString::fromLatin1(digestBytes.toHex());
    const QString contentDirectory = QStringLiteral("attachments/") + digestHex.left(2);
    if (!dataDirectory.mkpath(contentDirectory)) {
        QFile::remove(stagingPath);
        writeError(errorMessage,
                   /*% "The clipboard image could not be saved." */ qtTrId("craftward.clipboard_image.error.save"));
        return {};
    }

    const QString storedPath =
      dataDirectory.filePath(contentDirectory + QLatin1Char('/') + digestHex + QStringLiteral(".png"));
    if (QFileInfo::exists(storedPath)) {
        QFile::remove(stagingPath);
        return QUrl::fromLocalFile(storedPath);
    }
    if (QFile::rename(stagingPath, storedPath))
        return QUrl::fromLocalFile(storedPath);
    if (QFileInfo::exists(storedPath)) {
        QFile::remove(stagingPath);
        return QUrl::fromLocalFile(storedPath);
    }

    QFile::remove(stagingPath);
    writeError(errorMessage,
               /*% "The clipboard image could not be saved." */ qtTrId("craftward.clipboard_image.error.save"));
    return {};
}

QList<CodexAttachmentDescriptor>
CodexAttachmentInput::fromClipboard(QString* errorMessage)
{
    clearError(errorMessage);
    const QClipboard* clipboard = QGuiApplication::clipboard();
    const QMimeData* mimeData = clipboard == nullptr ? nullptr : clipboard->mimeData();
    if (mimeData == nullptr)
        return {};
    return fromMimeData(
      *mimeData, QStandardPaths::writableLocation(QStandardPaths::AppLocalDataLocation), errorMessage);
}

QList<CodexAttachmentDescriptor>
CodexAttachmentInput::fromMimeData(const QMimeData& mimeData, const QString& dataRoot, QString* errorMessage)
{
    clearError(errorMessage);
    if (mimeData.hasUrls()) {
        QList<QUrl> localFiles;
        QSet<QString> seenUrls;
        for (const QUrl& url : mimeData.urls()) {
            if (!url.isLocalFile())
                continue;
            const QString key = url.adjusted(QUrl::NormalizePathSegments).toString();
            if (!seenUrls.contains(key)) {
                seenUrls.insert(key);
                localFiles.append(url);
            }
        }
        if (!localFiles.isEmpty()) {
            const std::optional<QList<CodexAttachmentDescriptor>> described = describe(localFiles, false, errorMessage);
            return described.value_or(QList<CodexAttachmentDescriptor>{});
        }
    }

    if (!mimeData.hasImage())
        return {};
    const QImage image = clipboardImage(mimeData);
    if (image.isNull()) {
        writeError(errorMessage,
                   /*% "The clipboard image could not be read." */ qtTrId("craftward.clipboard_image.error.read"));
        return {};
    }
    const QUrl storedImage = storeClipboardImage(image, dataRoot, errorMessage);
    if (storedImage.isEmpty())
        return {};
    std::optional<QList<CodexAttachmentDescriptor>> described = describe({ storedImage }, true, errorMessage);
    if (!described.has_value() || described->isEmpty())
        return {};
    described->front().nameKind = CodexAttachmentNameKind::PastedImage;
    return *described;
}

QString
CodexAttachmentInput::kindName(CodexAttachmentKind kind)
{
    switch (kind) {
        case CodexAttachmentKind::LocalImage:
            return QStringLiteral("localImage");
        case CodexAttachmentKind::LocalAudio:
            return QStringLiteral("localAudio");
        case CodexAttachmentKind::Mention:
            return QStringLiteral("mention");
    }
    return QStringLiteral("mention");
}

QString
CodexAttachmentInput::nameKindName(CodexAttachmentNameKind kind)
{
    switch (kind) {
        case CodexAttachmentNameKind::FileName:
            return QStringLiteral("fileName");
        case CodexAttachmentNameKind::PastedImage:
            return QStringLiteral("pastedImage");
    }
    return QStringLiteral("fileName");
}
