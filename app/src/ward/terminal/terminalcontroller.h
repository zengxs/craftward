// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QAbstractItemModel>
#include <QMap>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QtQml/qqmlregistration.h>

#include <memory>

class QSettings;

// Owns local terminal processes for the lifetime of their conversation in this app run.
class TerminalController : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("TerminalController is provided by the application.")
    Q_PROPERTY(bool available READ available NOTIFY conversationChanged)
    Q_PROPERTY(bool panelVisible READ panelVisible NOTIFY viewChanged)
    Q_PROPERTY(QAbstractItemModel* tabs READ tabs NOTIFY viewChanged)
    Q_PROPERTY(QObject* activeSession READ activeSession NOTIFY viewChanged)
    Q_PROPERTY(int activeTabIndex READ activeTabIndex NOTIFY viewChanged)
    Q_PROPERTY(QString errorMessage READ errorMessage NOTIFY viewChanged)
    Q_PROPERTY(QStringList fontFamilies READ fontFamilies NOTIFY fontFamiliesChanged)
    Q_PROPERTY(QStringList bundledFontFamilies READ bundledFontFamilies CONSTANT)
    Q_PROPERTY(QString fontFamily READ fontFamily WRITE setFontFamily NOTIFY fontChanged)
    Q_PROPERTY(int fontSize READ fontSize WRITE setFontSize NOTIFY fontChanged)
    Q_PROPERTY(int panelHeight READ panelHeight WRITE setPanelHeight NOTIFY panelHeightChanged)

  public:
    struct ShellCommand
    {
        QString program;
        QStringList arguments;
        // Overrides inherited variables for terminal child processes only.
        QMap<QString, QString> environment;
    };

    // Use the account's interactive login shell unless a command is supplied explicitly.
    explicit TerminalController(QSettings& settings, QObject* parent = nullptr);
    TerminalController(QSettings& settings, ShellCommand shell, QObject* parent = nullptr);
    ~TerminalController() override;

    void selectConversation(const QString& threadId, const QString& workingDirectory);
    bool available() const;
    bool panelVisible() const;
    QAbstractItemModel* tabs() const;
    QObject* activeSession() const;
    int activeTabIndex() const;
    QString errorMessage() const;
    QStringList fontFamilies() const;
    QStringList bundledFontFamilies() const;
    QString fontFamily() const;
    int fontSize() const;
    int panelHeight() const;
    void setFontFamily(const QString& family);
    void setFontSize(int size);
    void setPanelHeight(int height);

    Q_INVOKABLE void togglePanel();
    Q_INVOKABLE void createTerminal();
    Q_INVOKABLE void activateTab(int index);
    Q_INVOKABLE void closeTab(int index);

  signals:
    void conversationChanged();
    void viewChanged();
    void fontFamiliesChanged();
    void fontChanged();
    void panelHeightChanged();
    void focusRequested();

  private:
    struct Private;
    std::unique_ptr<Private> d;
};
