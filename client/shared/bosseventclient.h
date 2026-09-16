#pragma once

#include <QByteArray>
#include <QLocalSocket>
#include <QObject>
#include <QString>
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

public:
    explicit BossEventClient(
        QObject *parent = nullptr
    );

    bool connected() const;
    QString bossUiState() const;

    Q_INVOKABLE void connectToBoss();

signals:
    void connectedChanged();
    void bossUiStateChanged();

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

    void scheduleReconnect();
    void resetReconnect();

    QLocalSocket m_socket;
    QByteArray m_buffer;

    QString m_bossUiState =
        QStringLiteral("closed");

    QTimer m_reconnectTimer;
    int m_reconnectDelayMs = 500;
};
