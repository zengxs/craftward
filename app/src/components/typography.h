// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#ifndef CRAFTWARD_TYPOGRAPHY_H
#define CRAFTWARD_TYPOGRAPHY_H

#include <QFont>
#include <QObject>
#include <QQmlEngine>
#include <QString>

class Typography : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(QString monoFamily READ monoFamily CONSTANT)
    Q_PROPERTY(QFont codeFont READ codeFont CONSTANT)
    Q_PROPERTY(qreal codeLineHeightScale READ codeLineHeightScale CONSTANT)

  public:
    explicit Typography(QObject* parent = nullptr);

    QString monoFamily() const;
    QFont codeFont() const;
    qreal codeLineHeightScale() const;

  private:
    QString m_monoFamily;
    QFont m_codeFont;
};

#endif
