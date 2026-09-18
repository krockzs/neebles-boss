#include "bosscontroller.h"

#include <QCoreApplication>
#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDir>
#include <QFileInfo>
#include <QGuiApplication>
#include <QIcon>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLocalSocket>
#include <QProcessEnvironment>
#include <QQmlApplicationEngine>
#include <QQmlContext>

static constexpr auto BOSS_DBUS_SERVICE = "org.neebles.Boss";

static QString bossSocketPath()
{
    const QString overridePath =
        QProcessEnvironment::systemEnvironment()
            .value(
                QStringLiteral(
                    "NEEBLES_SOCKET"
                )
            )
            .trimmed();

    if (!overridePath.isEmpty())
        return overridePath;

    return QStringLiteral(
        "/run/neebles/neebles.sock"
    );
}

static bool acquireBossUiLease(
    QLocalSocket &socket
)
{
    socket.connectToServer(
        bossSocketPath()
    );

    if (!socket.waitForConnected(3000))
        return false;

    const QJsonObject request{
        {
            QStringLiteral("target"),
            QStringLiteral("events")
        },
        {
            QStringLiteral("action"),
            QStringLiteral("lease")
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
                    QStringLiteral("boss-ui")
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

    if (socket.write(payload) != payload.size())
        return false;

    if (!socket.waitForBytesWritten(3000))
        return false;

    if (!socket.waitForReadyRead(3000))
        return false;

    const QByteArray response =
        socket.readLine().trimmed();

    if (response.isEmpty())
        return false;

    const QJsonDocument document =
        QJsonDocument::fromJson(response);

    if (!document.isObject())
        return false;

    const QJsonObject object =
        document.object();

    return
        object.value(
            QStringLiteral("type")
        ).toString()
        == QStringLiteral("subscribed");
}

static QString findBrandingAsset(const QString &name)
{
    if (
        qEnvironmentVariableIsSet(
            "NEEBLES_CLIENT_ROOT"
        )
    ) {
        const QString clientRoot =
            qEnvironmentVariable(
                "NEEBLES_CLIENT_ROOT"
            ).trimmed();

        if (clientRoot.isEmpty())
            return {};

        const QString path =
            QDir(clientRoot).filePath(
                QStringLiteral(
                    "assets/branding/"
                ) + name
            );

        if (QFileInfo::exists(path))
            return QFileInfo(path)
                .absoluteFilePath();

        return {};
    }

    const QStringList candidates = {
        QDir(
            QCoreApplication::applicationDirPath()
        ).filePath(
            QStringLiteral(
                "../assets/branding/"
            ) + name
        ),

        QDir::current().filePath(
            QStringLiteral(
                "client/assets/branding/"
            ) + name
        ),

        QStringLiteral(
            "/opt/neebles/client/assets/branding/"
        ) + name
    };

    for (const QString &path : candidates) {
        if (QFileInfo::exists(path))
            return QFileInfo(path)
                .absoluteFilePath();
    }

    return {};
}

int main(int argc, char *argv[])
{
    QGuiApplication app(argc, argv);

    app.setApplicationName(
        QStringLiteral("N.E.E.B.L.E.S. Boss")
    );

    app.setApplicationVersion(
        QStringLiteral("1.0.15")
    );

    app.setOrganizationName(
        QStringLiteral("N.E.E.B.L.E.S.")
    );

    app.setDesktopFileName(
        QStringLiteral("org.neebles.Boss")
    );

    QDBusConnection bus =
        QDBusConnection::sessionBus();

    if (!bus.isConnected())
        return 1;

    if (!bus.registerService(
            QString::fromLatin1(BOSS_DBUS_SERVICE)
        )) {
        return 0;
    }

    QLocalSocket bossUiLease;

    if (!acquireBossUiLease(bossUiLease)) {
        bus.unregisterService(
            QString::fromLatin1(
                BOSS_DBUS_SERVICE
            )
        );

        return 1;
    }

    const QString iconPath =
        findBrandingAsset(
            QStringLiteral(
                "neebles-boss-icon.png"
            )
        );

    if (!iconPath.isEmpty())
        app.setWindowIcon(QIcon(iconPath));

    BossController boss;

    QQmlApplicationEngine engine;

    engine.rootContext()->setContextProperty(
        QStringLiteral("boss"),
        &boss
    );

    engine.loadFromModule(
        "NeeblesUI",
        "Main"
    );

    if (engine.rootObjects().isEmpty()) {
        bus.unregisterService(
            QString::fromLatin1(
                BOSS_DBUS_SERVICE
            )
        );

        return -1;
    }

    const int result = app.exec();

    bus.unregisterService(
        QString::fromLatin1(
            BOSS_DBUS_SERVICE
        )
    );

    return result;
}
