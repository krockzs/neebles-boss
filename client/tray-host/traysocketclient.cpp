#include "traysocketclient.h"

#include <QJsonDocument>
#include <QJsonObject>
#include <QProcessEnvironment>
#include <QStandardPaths>

TrayModel::TrayModel(QObject *parent)
    : QAbstractListModel(parent)
{
}

int TrayModel::rowCount(
    const QModelIndex &parent
) const
{
    if (parent.isValid()) {
        return 0;
    }

    return m_items.size();
}

QVariant TrayModel::data(
    const QModelIndex &index,
    int role
) const
{
    if (
        !index.isValid()
        || index.row() < 0
        || index.row() >= m_items.size()
    ) {
        return {};
    }

    const TrayItem &item =
        m_items.at(index.row());

    switch (role) {
    case TrayIdRole:
        return item.trayId;

    case OwnerModuleRole:
        return item.ownerModule;

    case ModuleVersionRole:
        return item.moduleVersion;

    case IconRole:
        return item.icon;

    case ProviderRole:
        return item.provider;

    case ProtocolRole:
        return item.protocol;

    case VisibleRole:
        return item.visible;

    case OpenedRole:
        return item.opened;

    case WidthRole:
        return item.width;

    case HeightRole:
        return item.height;

    case StateRole:
        return item.state;

    default:
        return {};
    }
}

QHash<int, QByteArray>
TrayModel::roleNames() const
{
    return {
        {TrayIdRole, "trayId"},
        {OwnerModuleRole, "ownerModule"},
        {ModuleVersionRole, "moduleVersion"},
        {IconRole, "icon"},
        {ProviderRole, "provider"},
        {ProtocolRole, "protocol"},
        {VisibleRole, "trayVisible"},
        {OpenedRole, "opened"},
        {WidthRole, "contentWidth"},
        {HeightRole, "contentHeight"},
        {StateRole, "trayState"},
    };
}

bool TrayModel::openedForTray(
    const QString &trayId
) const
{
    const int trayIndex =
        indexOf(trayId);

    if (trayIndex < 0) {
        return false;
    }

    return m_items
        .at(trayIndex)
        .opened;
}

bool TrayModel::visibleForTray(
    const QString &trayId
) const
{
    const int trayIndex =
        indexOf(trayId);

    if (trayIndex < 0) {
        return false;
    }

    return m_items
        .at(trayIndex)
        .visible;
}

bool TrayModel::containsTray(
    const QString &trayId
) const
{
    return indexOf(trayId) >= 0;
}

int TrayModel::indexOf(
    const QString &trayId
) const
{
    for (
        int index = 0;
        index < m_items.size();
        ++index
    ) {
        if (
            m_items.at(index).trayId
            == trayId
        ) {
            return index;
        }
    }

    return -1;
}

void TrayModel::replaceSnapshot(
    const QVector<TrayItem> &items
)
{
    beginResetModel();

    m_items = items;

    endResetModel();
}

void TrayModel::upsert(
    const TrayItem &item
)
{
    const int existing =
        indexOf(item.trayId);

    if (existing >= 0) {
        m_items[existing] = item;

        emit dataChanged(
            index(existing),
            index(existing)
        );

        return;
    }

    const int position =
        m_items.size();

    beginInsertRows(
        QModelIndex(),
        position,
        position
    );

    m_items.push_back(item);

    endInsertRows();
}

void TrayModel::removeTray(
    const QString &trayId
)
{
    const int existing =
        indexOf(trayId);

    if (existing < 0) {
        return;
    }

    beginRemoveRows(
        QModelIndex(),
        existing,
        existing
    );

    m_items.removeAt(existing);

    endRemoveRows();
}

TraySocketClient::TraySocketClient(
    QObject *parent
)
    : QObject(parent)
    , m_model(this)
{
    connect(
        &m_socket,
        &QLocalSocket::connected,
        this,
        &TraySocketClient::onConnected
    );

    connect(
        &m_socket,
        &QLocalSocket::disconnected,
        this,
        &TraySocketClient::onDisconnected
    );

    connect(
        &m_socket,
        &QLocalSocket::readyRead,
        this,
        &TraySocketClient::onReadyRead
    );

    connect(
        &m_socket,
        &QLocalSocket::errorOccurred,
        this,
        [this](
            QLocalSocket::LocalSocketError
        ) {
            setError(
                m_socket.errorString()
            );

            scheduleReconnect();
        }
    );

    m_reconnectTimer.setSingleShot(true);

    connect(
        &m_reconnectTimer,
        &QTimer::timeout,
        this,
        [this]() {
            connectToManager();
        }
    );
}

TrayModel *TraySocketClient::model()
{
    return &m_model;
}

bool TraySocketClient::connected() const
{
    return
        m_socket.state()
        == QLocalSocket::ConnectedState;
}

QString TraySocketClient::error() const
{
    return m_error;
}

QString TraySocketClient::socketPath() const
{
    const QString overridePath =
        QProcessEnvironment::systemEnvironment()
            .value(
                QStringLiteral(
                    "NEEBLES_TRAY_SOCKET"
                )
            );

    if (!overridePath.isEmpty()) {
        return overridePath;
    }

    const QString runtime =
        QStandardPaths::writableLocation(
            QStandardPaths::RuntimeLocation
        );

    return runtime
        + QStringLiteral(
            "/neebles/tray.sock"
        );
}

void TraySocketClient::scheduleReconnect()
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

void TraySocketClient::resetReconnect()
{
    m_reconnectTimer.stop();
    m_reconnectDelayMs = 500;
}

void TraySocketClient::connectToManager()
{
    if (
        m_socket.state()
        != QLocalSocket::UnconnectedState
    ) {
        return;
    }

    setError({});

    m_socket.connectToServer(
        socketPath()
    );
}

void TraySocketClient::onConnected()
{
    resetReconnect();

    emit connectedChanged();

    setError({});

    sendSubscribe();
}

void TraySocketClient::onDisconnected()
{
    m_buffer.clear();

    m_model.replaceSnapshot(
        {}
    );

    emit connectedChanged();

    scheduleReconnect();
}

void TraySocketClient::sendCommand(
    const QVariantMap &command
)
{
    if (
        m_socket.state()
        != QLocalSocket::ConnectedState
    ) {
        setError(
            QStringLiteral(
                "Tray Host is not connected"
            )
        );

        return;
    }

    QByteArray payload =
        QJsonDocument(
            QJsonObject::fromVariantMap(
                command
            )
        ).toJson(
            QJsonDocument::Compact
        );

    payload.append('\n');

    m_socket.write(payload);
    m_socket.flush();
}

void TraySocketClient::openTray(
    const QString &trayId
)
{
    sendCommand({
        {
            QStringLiteral("type"),
            QStringLiteral("open")
        },
        {
            QStringLiteral("tray_id"),
            trayId
        }
    });
}

void TraySocketClient::closeTray(
    const QString &trayId
)
{
    sendCommand({
        {
            QStringLiteral("type"),
            QStringLiteral("close")
        },
        {
            QStringLiteral("tray_id"),
            trayId
        }
    });
}

void TraySocketClient::focusTray(
    const QString &trayId
)
{
    sendCommand({
        {
            QStringLiteral("type"),
            QStringLiteral("focus")
        },
        {
            QStringLiteral("tray_id"),
            trayId
        }
    });
}

void TraySocketClient::setTrayVisibility(
    const QString &trayId,
    bool visible
)
{
    QLocalSocket controlSocket;

    controlSocket.connectToServer(
        socketPath()
    );

    if (!controlSocket.waitForConnected(1000)) {
        setError(
            QStringLiteral(
                "Tray control connection failed: %1"
            ).arg(
                controlSocket.errorString()
            )
        );

        return;
    }

    QByteArray payload =
        QJsonDocument(
            QJsonObject{
                {
                    QStringLiteral("type"),
                    QStringLiteral("set_visibility")
                },
                {
                    QStringLiteral("tray_id"),
                    trayId
                },
                {
                    QStringLiteral("visible"),
                    visible
                }
            }
        ).toJson(
            QJsonDocument::Compact
        );

    payload.append('\n');

    controlSocket.write(payload);

    if (!controlSocket.waitForBytesWritten(1000)) {
        setError(
            QStringLiteral(
                "Tray control write failed: %1"
            ).arg(
                controlSocket.errorString()
            )
        );

        return;
    }

    if (!controlSocket.waitForReadyRead(1000)) {
        setError(
            QStringLiteral(
                "Tray control response timeout"
            )
        );

        return;
    }

    const QByteArray responseLine =
        controlSocket.readLine().trimmed();

    const QJsonDocument response =
        QJsonDocument::fromJson(
            responseLine
        );

    if (!response.isObject()) {
        setError(
            QStringLiteral(
                "Invalid Tray control response"
            )
        );

        return;
    }

    const QJsonObject object =
        response.object();

    if (
        object.value(
            QStringLiteral("type")
        ).toString()
        == QStringLiteral("error")
    ) {
        setError(
            object.value(
                QStringLiteral("message")
            ).toString(
                QStringLiteral(
                    "Tray visibility change failed"
                )
            )
        );

        return;
    }

    setError(QString());

    controlSocket.disconnectFromServer();
}

void TraySocketClient::sendSubscribe()
{
    const QJsonObject request{
        {
            QStringLiteral("type"),
            QStringLiteral("subscribe")
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

TrayItem TraySocketClient::trayFromVariant(
    const QVariantMap &value
) const
{
    TrayItem item;

    item.trayId =
        value.value(
            QStringLiteral("tray_id")
        ).toString();

    item.ownerModule =
        value.value(
            QStringLiteral("owner_module")
        ).toString();

    item.moduleVersion =
        value.value(
            QStringLiteral("module_version")
        ).toString();

    item.icon =
        value.value(
            QStringLiteral("icon")
        ).toString();

    item.provider =
        value.value(
            QStringLiteral("provider")
        ).toString();

    item.protocol =
        value.value(
            QStringLiteral("protocol")
        ).toInt();

    item.visible =
        value.value(
            QStringLiteral("visible")
        ).toBool();

    item.opened =
        value.value(
            QStringLiteral("opened")
        ).toBool();

    item.width =
        value.value(
            QStringLiteral("width")
        ).toInt();

    item.height =
        value.value(
            QStringLiteral("height")
        ).toInt();

    item.state =
        value.value(
            QStringLiteral("state")
        ).toMap();

    return item;
}

void TraySocketClient::processLine(
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
        setError(
            QStringLiteral(
                "Invalid tray protocol JSON: %1"
            ).arg(
                parseError.errorString()
            )
        );

        return;
    }

    const QVariantMap message =
        document.object()
            .toVariantMap();

    const QString type =
        message.value(
            QStringLiteral("type")
        ).toString();

    if (type == QStringLiteral("snapshot")) {
        QVector<TrayItem> items;

        const QVariantList trays =
            message.value(
                QStringLiteral("trays")
            ).toList();

        items.reserve(
            trays.size()
        );

        for (
            const QVariant &value :
            trays
        ) {
            items.push_back(
                trayFromVariant(
                    value.toMap()
                )
            );
        }

        m_model.replaceSnapshot(
            items
        );

        return;
    }

    if (type == QStringLiteral("event")) {
        const QVariantMap event =
            message.value(
                QStringLiteral("event")
            ).toMap();

        const QString eventType =
            event.value(
                QStringLiteral("event")
            ).toString();

        if (
            eventType
            == QStringLiteral("registered")
            || eventType
            == QStringLiteral("updated")
        ) {
            m_model.upsert(
                trayFromVariant(
                    event.value(
                        QStringLiteral("tray")
                    ).toMap()
                )
            );

            return;
        }

        if (
            eventType
            == QStringLiteral("unregistered")
        ) {
            m_model.removeTray(
                event.value(
                    QStringLiteral("tray_id")
                ).toString()
            );

            return;
        }

        setError(
            QStringLiteral(
                "Unknown tray event: %1"
            ).arg(eventType)
        );

        return;
    }

    if (type == QStringLiteral("ack")) {
        return;
    }

    if (type == QStringLiteral("error")) {
        setError(
            message.value(
                QStringLiteral("message")
            ).toString()
        );

        return;
    }

    setError(
        QStringLiteral(
            "Unexpected tray protocol message: %1"
        ).arg(type)
    );
}

void TraySocketClient::onReadyRead()
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

        if (line.isEmpty()) {
            continue;
        }

        processLine(line);
    }
}

void TraySocketClient::setError(
    const QString &value
)
{
    if (m_error == value) {
        return;
    }

    m_error = value;

    emit errorChanged();
}
