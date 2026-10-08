#pragma once

#include <QObject>
#include <QProcess>
#include <QString>
#include <QVariantList>

class BossCommandClient final : public QObject
{
    Q_OBJECT

    Q_PROPERTY(
        QVariantList installedModules
        READ installedModules
        NOTIFY installedModulesChanged
    )

    Q_PROPERTY(
        bool busy
        READ busy
        NOTIFY busyChanged
    )

    Q_PROPERTY(
        QString error
        READ error
        NOTIFY errorChanged
    )

public:
    explicit BossCommandClient(
        QObject *parent = nullptr
    );

    QVariantList installedModules() const;
    bool busy() const;
    QString error() const;

    Q_INVOKABLE bool startBoss();

    Q_INVOKABLE void refreshModules();

    Q_INVOKABLE bool surfaceAction(
        const QString &moduleName,
        const QString &itemId,
        const QString &action
    );

    Q_INVOKABLE bool setModuleEnabled(
        const QString &moduleName,
        bool enabled
    );

signals:
    void installedModulesChanged();
    void busyChanged();
    void errorChanged();

private:
    QString commandPath() const;

    void setBusy(bool value);
    void setError(const QString &value);

    QVariantList m_installedModules;
    bool m_busy = false;
    QString m_error;

    QProcess m_actionProcess;
};
