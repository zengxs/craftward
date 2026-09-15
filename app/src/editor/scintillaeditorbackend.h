// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#ifndef CRAFTWARD_SCINTILLAEDITORBACKEND_H
#define CRAFTWARD_SCINTILLAEDITORBACKEND_H

#include "scintillaimageitem_p.h"
#include <QColor>
#include <QObject>
#include <QQmlEngine>
#include <QString>
#include <QVariantList>

#include <memory>

class ScintillaEditorBackendPrivate;

class ScintillaEditorBackend : public ScintillaImageItem
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(qreal verticalPosition READ verticalPosition WRITE setVerticalPosition NOTIFY scrollChanged)
    Q_PROPERTY(qreal verticalSize READ verticalSize NOTIFY scrollChanged)
    Q_PROPERTY(qreal horizontalPosition READ horizontalPosition WRITE setHorizontalPosition NOTIFY scrollChanged)
    Q_PROPERTY(qreal horizontalSize READ horizontalSize NOTIFY scrollChanged)
    Q_PROPERTY(bool canUndo READ canUndo NOTIFY editorStateChanged)
    Q_PROPERTY(bool canRedo READ canRedo NOTIFY editorStateChanged)
    Q_PROPERTY(bool hasSelection READ hasSelection NOTIFY editorStateChanged)
    Q_PROPERTY(bool canPaste READ canPaste NOTIFY editorStateChanged)
    Q_PROPERTY(QString text READ text WRITE setText NOTIFY textChanged)
    Q_PROPERTY(bool readOnly READ isReadOnly WRITE setReadOnly NOTIFY readOnlyChanged)
    Q_PROPERTY(bool wordWrap READ wordWrap WRITE setWordWrap NOTIFY wordWrapChanged)
    Q_PROPERTY(QString fontFamily READ fontFamily WRITE setFontFamily NOTIFY fontFamilyChanged)
    Q_PROPERTY(qreal fontPointSize READ fontPointSize WRITE setFontPointSize NOTIFY fontPointSizeChanged)
    Q_PROPERTY(int fontWeight READ fontWeight WRITE setFontWeight NOTIFY fontWeightChanged)
    Q_PROPERTY(qreal lineHeightScale READ lineHeightScale WRITE setLineHeightScale NOTIFY lineHeightScaleChanged)
    Q_PROPERTY(QColor foregroundColor READ foregroundColor WRITE setForegroundColor NOTIFY foregroundColorChanged)
    Q_PROPERTY(QColor backgroundColor READ backgroundColor WRITE setBackgroundColor NOTIFY backgroundColorChanged)
    Q_PROPERTY(QColor selectionForegroundColor READ selectionForegroundColor WRITE setSelectionForegroundColor NOTIFY
                 selectionForegroundColorChanged)
    Q_PROPERTY(QColor selectionBackgroundColor READ selectionBackgroundColor WRITE setSelectionBackgroundColor NOTIFY
                 selectionBackgroundColorChanged)

  public:
    explicit ScintillaEditorBackend(QQuickItem* parent = nullptr);
    ~ScintillaEditorBackend() override;

    // Scintilla message parameters use UTF-8 byte positions, not QString indices.
    qintptr sendMessage(unsigned int message, quintptr wParam = 0, qintptr lParam = 0);
    qreal verticalPosition() const;
    void setVerticalPosition(qreal value);
    qreal verticalSize() const;
    qreal horizontalPosition() const;
    void setHorizontalPosition(qreal value);
    qreal horizontalSize() const;
    bool canUndo() const;
    bool canRedo() const;
    bool hasSelection() const;
    bool canPaste() const;
    Q_INVOKABLE void undo();
    Q_INVOKABLE void redo();
    Q_INVOKABLE void cut();
    Q_INVOKABLE void copy();
    Q_INVOKABLE void paste();
    Q_INVOKABLE void selectAll();
    Q_INVOKABLE void deleteSelection();
    QVariant inputMethodQuery(Qt::InputMethodQuery query) const override;

    QString text() const;
    void setText(const QString& text);
    Q_INVOKABLE void revealLocation(int startLine, int endLine = 0);

    bool isReadOnly() const;
    void setReadOnly(bool readOnly);

    bool wordWrap() const;
    void setWordWrap(bool wordWrap);

    QString fontFamily() const;
    void setFontFamily(const QString& fontFamily);

    qreal fontPointSize() const;
    void setFontPointSize(qreal fontPointSize);

    int fontWeight() const;
    void setFontWeight(int fontWeight);

    qreal lineHeightScale() const;
    void setLineHeightScale(qreal lineHeightScale);

    QColor foregroundColor() const;
    void setForegroundColor(const QColor& foregroundColor);

    QColor backgroundColor() const;
    void setBackgroundColor(const QColor& backgroundColor);

    QColor selectionForegroundColor() const;
    void setSelectionForegroundColor(const QColor& selectionForegroundColor);

    QColor selectionBackgroundColor() const;
    void setSelectionBackgroundColor(const QColor& selectionBackgroundColor);

  signals:
    void scrollChanged();
    void editorStateChanged();
    void contextMenuRequested(QPointF position, QVariantList entries);
    void textChanged();
    void readOnlyChanged();
    void wordWrapChanged();
    void fontFamilyChanged();
    void fontPointSizeChanged();
    void fontWeightChanged();
    void lineHeightScaleChanged();
    void foregroundColorChanged();
    void backgroundColorChanged();
    void selectionForegroundColorChanged();
    void selectionBackgroundColorChanged();

  protected:
    bool paintImage(QPainter& painter, const QRect& rect) override;
    void geometryChange(const QRectF& geometry, const QRectF& oldGeometry) override;
    void itemChange(ItemChange change, const ItemChangeData& data) override;
    bool event(QEvent* event) override;
    void keyPressEvent(QKeyEvent* event) override;
    void mousePressEvent(QMouseEvent* event) override;
    void mouseMoveEvent(QMouseEvent* event) override;
    void mouseReleaseEvent(QMouseEvent* event) override;
    void mouseDoubleClickEvent(QMouseEvent* event) override;
    void mouseUngrabEvent() override;
    void hoverMoveEvent(QHoverEvent* event) override;
    void hoverLeaveEvent(QHoverEvent* event) override;
    void wheelEvent(QWheelEvent* event) override;
    void focusInEvent(QFocusEvent* event) override;
    void focusOutEvent(QFocusEvent* event) override;
    void inputMethodEvent(QInputMethodEvent* event) override;
    void dragEnterEvent(QDragEnterEvent* event) override;
    void dragMoveEvent(QDragMoveEvent* event) override;
    void dragLeaveEvent(QDragLeaveEvent* event) override;
    void dropEvent(QDropEvent* event) override;

  private:
    friend class ScintillaEditorBackendPrivate;

    std::unique_ptr<ScintillaEditorBackendPrivate> d;
};

#endif
