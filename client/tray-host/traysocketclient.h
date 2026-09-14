#pragma once

#include <QAbstractListModel>
#include <QByteArray>
#include <QHash>
#include <QLocalSocket>
#include <QObject>
#include <QString>
#include <QTimer>
#include <QVariantMap>
#include <QVector>

struct TrayItem {
    QString trayId;
    QString ownerModule;
    QString moduleVersion;
    QString icon;
    QString provider;
    int protocol = 0;
    bool visible = false;
    bool opened = false;
    int width = 0;
    int height = 0;
    QVariantMap state;
};

class TrayModel final : public QAbstractListModel
{
    Q_OBJECT

public:
    enum Role {
        TrayIdRole = Qt::UserRole + 1,
        OwnerModuleRole,
        ModuleVersionRole,
        IconRole,
        ProviderRole,
        ProtocolRole,
        VisibleRole,
        OpenedRole,
        WidthRole,
        HeightRole,
        StateRole
    };

    explicit TrayModel(QObject *parent = nullptr);

    int rowCount(
        const QModelIndex &parent = QModelIndex()
    ) const override;

    QVariant data(
        const QModelIndex &index,
        int role
    ) const override;

    QHash<int, QByteArray>
    roleNames() const override;

    Q_INVOKABLE bool openedForTray(
        const QString &trayId
    ) const;

    Q_INVOKABLE bool containsTray(
        const QString &trayId
    ) const;

    void replaceSnapshot(
        const QVector<TrayItem> &items
    );

    void upsert(
        const TrayItem &item
    );

    void removeTray(
        const QString &trayId
    );

private:
    int indexOf(
        const QString &trayId
    ) const;

    QVector<TrayItem> m_items;
};

class TraySocketClient final : public QObject
{
    Q_OBJECT

    Q_PROPERTY(
        TrayModel* model
        READ model
        CONSTANT
    )

    Q_PROPERTY(
        bool connected
        READ connected
        NOTIFY connectedChanged
    )

    Q_PROPERTY(
        QString error
        READ error
        NOTIFY errorChanged
    )

public:
    explicit TraySocketClient(
        QObject *parent = nullptr
    );

    TrayModel *model();

    bool connected() const;

    QString error() const;

    Q_INVOKABLE void connectToManager();

    Q_INVOKABLE void openTray(
        const QString &trayId
    );

    Q_INVOKABLE void closeTray(
        const QString &trayId
    );

    Q_INVOKABLE void focusTray(
        const QString &trayId
    );

signals:
    void connectedChanged();
    void errorChanged();

private slots:
    void onConnected();
    void onDisconnected();
    void onReadyRead();

private:
    QString socketPath() const;

    void sendSubscribe();

    void sendCommand(
        const QVariantMap &command
    );

    void processLine(
        const QByteArray &line
    );

    TrayItem trayFromVariant(
        const QVariantMap &value
    ) const;

    void setError(
        const QString &value
    );

    void scheduleReconnect();
    void resetReconnect();

    QLocalSocket m_socket;
    QByteArray m_buffer;
    TrayModel m_model;
    QString m_error;

    QTimer m_reconnectTimer;
    int m_reconnectDelayMs = 500;
};
