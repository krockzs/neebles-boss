#include "bosscontroller.h"

#include <QCoreApplication>
#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDir>
#include <QFileInfo>
#include <QGuiApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQmlContext>

static constexpr auto BOSS_DBUS_SERVICE = "org.neebles.Boss";

static QString findBrandingAsset(const QString &name)
{
    const QString clientRoot = qEnvironmentVariable("NEEBLES_CLIENT_ROOT");

    const QStringList candidates = {
        clientRoot.isEmpty()
            ? QString()
            : QDir(clientRoot).filePath(
                  QStringLiteral("assets/branding/") + name
              ),

        QDir(QCoreApplication::applicationDirPath()).filePath(
            QStringLiteral("../assets/branding/") + name
        ),

        QDir::current().filePath(
            QStringLiteral("client/assets/branding/") + name
        ),

        QStringLiteral(
            "/opt/neebles/client/assets/branding/"
        ) + name
    };

    for (const QString &path : candidates) {
        if (!path.isEmpty() && QFileInfo::exists(path))
            return QFileInfo(path).absoluteFilePath();
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
        QStringLiteral("1.0.4")
    );

    app.setOrganizationName(
        QStringLiteral("N.E.E.B.L.E.S.")
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

    const QString iconPath =
        findBrandingAsset(
            QStringLiteral(
                "neebles-boss-launcher-icon.png"
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
