#include "bosscontroller.h"

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QGuiApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQmlContext>

static QString findBrandingAsset(const QString &name)
{
    const QString clientRoot = qEnvironmentVariable("NEEBLES_CLIENT_ROOT");
    const QStringList candidates = {
        clientRoot.isEmpty() ? QString() : QDir(clientRoot).filePath(QStringLiteral("assets/branding/") + name),
        QDir(QCoreApplication::applicationDirPath()).filePath(QStringLiteral("../assets/branding/") + name),
        QDir::current().filePath(QStringLiteral("client/assets/branding/") + name),
        QStringLiteral("/opt/neebles/client/assets/branding/") + name
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
    app.setApplicationName(QStringLiteral("N.E.E.B.L.E.S. Boss"));
    app.setApplicationVersion(QStringLiteral("1.0.0"));
    app.setOrganizationName(QStringLiteral("N.E.E.B.L.E.S."));

    const QString iconPath = findBrandingAsset(QStringLiteral("neebles-boss-launcher-icon.png"));
    if (!iconPath.isEmpty())
        app.setWindowIcon(QIcon(iconPath));

    BossController boss;

    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty(QStringLiteral("boss"), &boss);
    engine.loadFromModule("NeeblesUI", "Main");

    if (engine.rootObjects().isEmpty())
        return -1;

    return app.exec();
}
