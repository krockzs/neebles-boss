#include "bosseventclient.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcessEnvironment>

BossEventClient::BossEventClient(
    QObject *parent
)
    : QObject(parent)
{
    connect(
        &m_socket,
        &QLocalSocket::connected,
        this,
        &BossEventClient::onConnected
    );

    connect(
        &m_socket,
        &QLocalSocket::disconnected,
        this,
        &BossEventClient::onDisconnected
    );

    connect(
        &m_socket,
        &QLocalSocket::readyRead,
        this,
        &BossEventClient::onReadyRead
    );

    connect(
        &m_socket,
        &QLocalSocket::errorOccurred,
        this,
        [this](
            QLocalSocket::LocalSocketError
        ) {
            scheduleReconnect();
        }
    );

    m_reconnectTimer.setSingleShot(true);

    connect(
        &m_reconnectTimer,
        &QTimer::timeout,
        this,
        [this]() {
            connectToBoss();
        }
    );
}

bool BossEventClient::connected() const
{
    return
        m_socket.state()
        == QLocalSocket::ConnectedState;
}

QString BossEventClient::bossUiState() const
{
    return m_bossUiState;
}

QString BossEventClient::socketPath() const
{
    const QString overridePath =
        QProcessEnvironment::systemEnvironment()
            .value(
                QStringLiteral(
                    "NEEBLES_SOCKET"
                )
            )
            .trimmed();

    if (!overridePath.isEmpty()) {
        return overridePath;
    }

    return QStringLiteral(
        "/run/neebles/neebles.sock"
    );
}

void BossEventClient::connectToBoss()
{
    if (
        m_socket.state()
        != QLocalSocket::UnconnectedState
    ) {
        return;
    }

    m_socket.connectToServer(
        socketPath()
    );
}

void BossEventClient::onConnected()
{
    resetReconnect();

    emit connectedChanged();

    sendSubscribe();
}

void BossEventClient::onDisconnected()
{
    m_buffer.clear();

    emit connectedChanged();

    scheduleReconnect();
}

void BossEventClient::sendSubscribe()
{
    const QJsonObject request{
        {
            QStringLiteral("target"),
            QStringLiteral("events")
        },
        {
            QStringLiteral("action"),
            QStringLiteral("subscribe")
        },
        {
            QStringLiteral("args"),
            QJsonArray{
                QStringLiteral("boss-ui")
            }
        },
        {
            QStringLiteral("context"),
            QJsonObject{
                {
                    QStringLiteral("caller"),
                    QStringLiteral("surface-client")
                }
            }
        }
    };

    QByteArray payload =
        QJsonDocument(request)
            .toJson(
                QJsonDocument::Compact
            );

    payload.append('\n');

    m_socket.write(payload);
    m_socket.flush();
}

void BossEventClient::setBossUiState(
    const QString &state
)
{
    if (
        state != QStringLiteral("closed")
        && state != QStringLiteral("opening")
        && state != QStringLiteral("open")
    ) {
        return;
    }

    if (m_bossUiState == state) {
        return;
    }

    m_bossUiState = state;

    emit bossUiStateChanged();
}

void BossEventClient::processLine(
    const QByteArray &line
)
{
    QJsonParseError parseError;

    const QJsonDocument document =
        QJsonDocument::fromJson(
            line,
            &parseError
        );

    if (
        parseError.error
            != QJsonParseError::NoError
        || !document.isObject()
    ) {
        return;
    }

    const QJsonObject message =
        document.object();

    const QString type =
        message.value(
            QStringLiteral("type")
        ).toString();

    if (
        type
        != QStringLiteral("event")
    ) {
        return;
    }

    const QString topic =
        message.value(
            QStringLiteral("topic")
        ).toString();

    if (
        topic
        != QStringLiteral("boss-ui")
    ) {
        return;
    }

    const QString event =
        message.value(
            QStringLiteral("event")
        ).toString();

    if (
        event
            != QStringLiteral(
                "state_snapshot"
            )
        && event
            != QStringLiteral(
                "state_changed"
            )
    ) {
        return;
    }

    const QString state =
        message.value(
            QStringLiteral("payload")
        )
        .toObject()
        .value(
            QStringLiteral("state")
        )
        .toString();

    setBossUiState(state);
}

void BossEventClient::onReadyRead()
{
    m_buffer.append(
        m_socket.readAll()
    );

    while (true) {
        const qsizetype newline =
            m_buffer.indexOf('\n');

        if (newline < 0) {
            break;
        }

        const QByteArray line =
            m_buffer
                .left(newline)
                .trimmed();

        m_buffer.remove(
            0,
            newline + 1
        );

        if (!line.isEmpty()) {
            processLine(line);
        }
    }
}

void BossEventClient::scheduleReconnect()
{
    if (
        m_socket.state()
        == QLocalSocket::ConnectedState
    ) {
        return;
    }

    if (m_reconnectTimer.isActive()) {
        return;
    }

    m_reconnectTimer.start(
        m_reconnectDelayMs
    );

    m_reconnectDelayMs =
        qMin(
            m_reconnectDelayMs * 2,
            2000
        );
}

void BossEventClient::resetReconnect()
{
    m_reconnectTimer.stop();
    m_reconnectDelayMs = 500;
}
