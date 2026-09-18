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

bool BossEventClient::launcherEnabled() const
{
    return m_launcherEnabled;
}

bool BossEventClient::trayEnabled() const
{
    return m_trayEnabled;
}

QStringList BossEventClient::hiddenLauncherModules() const
{
    return m_hiddenLauncherModules;
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
                QStringLiteral("boss-ui"),
                QStringLiteral("settings.boss")
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

void BossEventClient::setLauncherEnabled(
    bool enabled
)
{
    if (m_launcherEnabled == enabled) {
        return;
    }

    m_launcherEnabled = enabled;

    emit launcherEnabledChanged();
}

void BossEventClient::setTrayEnabled(
    bool enabled
)
{
    if (m_trayEnabled == enabled) {
        return;
    }

    m_trayEnabled = enabled;

    emit trayEnabledChanged();
}

void BossEventClient::setHiddenLauncherModules(
    const QStringList &modules
)
{
    if (
        m_hiddenLauncherModules
        == modules
    ) {
        return;
    }

    m_hiddenLauncherModules = modules;

    emit hiddenLauncherModulesChanged();
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

    const QJsonObject payload =
        message.value(
            QStringLiteral("payload")
        ).toObject();

    if (
        topic
            == QStringLiteral("boss-ui")
    ) {
        setBossUiState(
            payload.value(
                QStringLiteral("state")
            ).toString()
        );

        return;
    }

    if (
        topic
            == QStringLiteral("settings.boss")
    ) {
        if (
            payload.contains(
                QStringLiteral("launcher_enabled")
            )
        ) {
            setLauncherEnabled(
                payload.value(
                    QStringLiteral("launcher_enabled")
                ).toBool(true)
            );
        }

        if (
            payload.contains(
                QStringLiteral("tray_enabled")
            )
        ) {
            setTrayEnabled(
                payload.value(
                    QStringLiteral("tray_enabled")
                ).toBool(true)
            );
        }

        if (
            payload.contains(
                QStringLiteral(
                    "hidden_launcher_modules"
                )
            )
        ) {
            QStringList hiddenModules;

            const QJsonArray values =
                payload.value(
                    QStringLiteral(
                        "hidden_launcher_modules"
                    )
                ).toArray();

            for (
                const QJsonValue &value
                : values
            ) {
                if (value.isString()) {
                    hiddenModules.append(
                        value.toString()
                    );
                }
            }

            setHiddenLauncherModules(
                hiddenModules
            );
        }
    }
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
