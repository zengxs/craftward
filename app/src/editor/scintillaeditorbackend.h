// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#ifndef CRAFTWARD_SCINTILLAEDITORBACKEND_H
#define CRAFTWARD_SCINTILLAEDITORBACKEND_H

#include <QColor>
#include <QObject>
#include <QQmlEngine>
#include <QString>
#include <QWindow>

#include <memory>

class ScintillaEditorBackendPrivate;

class ScintillaEditorBackend : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QWindow* window READ window CONSTANT)
    Q_PROPERTY(QString text READ text WRITE setText NOTIFY textChanged)
    Q_PROPERTY(bool readOnly READ isReadOnly WRITE setReadOnly NOTIFY readOnlyChanged)
    Q_PROPERTY(bool wordWrap READ wordWrap WRITE setWordWrap NOTIFY wordWrapChanged)
    Q_PROPERTY(QString fontFamily READ fontFamily WRITE setFontFamily NOTIFY fontFamilyChanged)
    Q_PROPERTY(qreal fontPointSize READ fontPointSize WRITE setFontPointSize NOTIFY fontPointSizeChanged)
    Q_PROPERTY(QColor foregroundColor READ foregroundColor WRITE setForegroundColor NOTIFY foregroundColorChanged)
    Q_PROPERTY(QColor backgroundColor READ backgroundColor WRITE setBackgroundColor NOTIFY backgroundColorChanged)
    Q_PROPERTY(QColor selectionForegroundColor READ selectionForegroundColor WRITE setSelectionForegroundColor NOTIFY
                 selectionForegroundColorChanged)
    Q_PROPERTY(QColor selectionBackgroundColor READ selectionBackgroundColor WRITE setSelectionBackgroundColor NOTIFY
                 selectionBackgroundColorChanged)

  public:
    explicit ScintillaEditorBackend(QObject* parent = nullptr);
    ~ScintillaEditorBackend() override;

    QWindow* window() const;

    QString text() const;
    void setText(const QString& text);

    bool isReadOnly() const;
    void setReadOnly(bool readOnly);

    bool wordWrap() const;
    void setWordWrap(bool wordWrap);

    QString fontFamily() const;
    void setFontFamily(const QString& fontFamily);

    qreal fontPointSize() const;
    void setFontPointSize(qreal fontPointSize);

    QColor foregroundColor() const;
    void setForegroundColor(const QColor& foregroundColor);

    QColor backgroundColor() const;
    void setBackgroundColor(const QColor& backgroundColor);

    QColor selectionForegroundColor() const;
    void setSelectionForegroundColor(const QColor& selectionForegroundColor);

    QColor selectionBackgroundColor() const;
    void setSelectionBackgroundColor(const QColor& selectionBackgroundColor);

  signals:
    void textChanged();
    void readOnlyChanged();
    void wordWrapChanged();
    void fontFamilyChanged();
    void fontPointSizeChanged();
    void foregroundColorChanged();
    void backgroundColorChanged();
    void selectionForegroundColorChanged();
    void selectionBackgroundColorChanged();

  private:
    friend class ScintillaEditorBackendPrivate;
    void handleNativeTextChanged();

    std::unique_ptr<ScintillaEditorBackendPrivate> d;
};

#endif
