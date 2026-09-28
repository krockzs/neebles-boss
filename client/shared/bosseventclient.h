#pragma once

#include <QByteArray>
#include <QLocalSocket>
#include <QObject>
#include <QString>
#include <QStringList>
#include <QTimer>

class BossEventClient : public QObject
{
    Q_OBJECT

    Q_PROPERTY(
        bool connected
        READ connected
        NOTIFY connectedChanged
    )

    Q_PROPERTY(
        QString bossUiState
        READ bossUiState
        NOTIFY bossUiStateChanged
    )

    Q_PROPERTY(
        bool launcherEnabled
        READ launcherEnabled
        NOTIFY launcherEnabledChanged
    )

    Q_PROPERTY(
        bool trayEnabled
        READ trayEnabled
        NOTIFY trayEnabledChanged
    )

public:
    explicit BossEventClient(
        QObject *parent = nullptr
    );

    bool connected() const;
    QString bossUiState() const;
    bool launcherEnabled() const;
    bool trayEnabled() const;

    Q_INVOKABLE void connectToBoss();

signals:
    void connectedChanged();
    void bossUiStateChanged();
    void launcherEnabledChanged();
    void trayEnabledChanged();
    void settingsChanged();

private slots:
    void onConnected();
    void onDisconnected();
    void onReadyRead();

private:
    QString socketPath() const;

    void sendSubscribe();

    void processLine(
        const QByteArray &line
    );

    void setBossUiState(
        const QString &state
    );

    void setLauncherEnabled(
        bool enabled
    );

    void setTrayEnabled(
        bool enabled
    );

    void scheduleReconnect();
    void resetReconnect();

    QLocalSocket m_socket;
    QByteArray m_buffer;

    QString m_bossUiState =
        QStringLiteral("closed");

    bool m_launcherEnabled = true;
    bool m_trayEnabled = true;

    QTimer m_reconnectTimer;
    int m_reconnectDelayMs = 500;
};
