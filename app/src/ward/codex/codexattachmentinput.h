// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QByteArray>
#include <QList>
#include <QString>
#include <QUrl>

#include <optional>

class QImage;
class QMimeData;

enum class CodexAttachmentKind
{
    LocalImage,
    LocalAudio,
    Mention,
};

enum class CodexAttachmentNameKind
{
    FileName,
    PastedImage,
};

struct CodexTurnAttachment
{
    CodexAttachmentKind kind;
    QByteArray name;
    QByteArray path;
};

struct CodexAttachmentDescriptor
{
    QUrl url;
    QString name;
    QString mimeType;
    CodexAttachmentKind kind;
    bool managed;
    CodexAttachmentNameKind nameKind = CodexAttachmentNameKind::FileName;
};

class CodexAttachmentInput final
{
  public:
    [[nodiscard]] static std::optional<QList<CodexAttachmentDescriptor>> describe(const QList<QUrl>& attachments,
                                                                                  bool managed,
                                                                                  QString* errorMessage);
    [[nodiscard]] static std::optional<QList<CodexTurnAttachment>> prepare(const QList<QUrl>& attachments,
                                                                           QString* errorMessage);
    [[nodiscard]] static QList<CodexAttachmentDescriptor> fromClipboard(QString* errorMessage);
    [[nodiscard]] static QString kindName(CodexAttachmentKind kind);
    [[nodiscard]] static QString nameKindName(CodexAttachmentNameKind kind);

  private:
    friend class CodexHistoryControllerTest;

    [[nodiscard]] static QList<CodexAttachmentDescriptor> fromMimeData(const QMimeData& mimeData,
                                                                       const QString& dataRoot,
                                                                       QString* errorMessage);
    [[nodiscard]] static QUrl storeClipboardImage(const QImage& image, const QString& dataRoot, QString* errorMessage);
};
