#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QGuiApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QUrl>

#include "installercontroller.h"

static QString findBrandingAsset(const QString &name)
{
    const QString clientRoot = qEnvironmentVariable("NEEBLES_CLIENT_ROOT");
    const QStringList candidates = {
        clientRoot.isEmpty() ? QString() : QDir(clientRoot).filePath(QStringLiteral("assets/branding/") + name),
        QDir::current().filePath(QStringLiteral("client/assets/branding/") + name),
        QDir(QCoreApplication::applicationDirPath()).filePath(QStringLiteral("../../client/assets/branding/") + name),
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
    app.setApplicationName(QStringLiteral("N.E.E.B.L.E.S. Installer"));
    app.setApplicationVersion(QStringLiteral("0.0.1"));
    app.setOrganizationName(QStringLiteral("N.E.E.B.L.E.S."));

    const QString iconPath = findBrandingAsset(QStringLiteral("neebles-boss-launcher-icon.png"));
    if (!iconPath.isEmpty())
        app.setWindowIcon(QIcon(iconPath));

    InstallerController installer(app.arguments());

    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty(QStringLiteral("installer"), &installer);

    const QString wallPath = findBrandingAsset(QStringLiteral("neebles-boss-wall-transparent.png"));
    QUrl brandingRootUrl;
    if (!wallPath.isEmpty()) {
        QDir brandingDir = QFileInfo(wallPath).absoluteDir();
        brandingRootUrl = QUrl::fromLocalFile(brandingDir.absolutePath() + QDir::separator());
    }
    engine.rootContext()->setContextProperty(QStringLiteral("brandingRootUrl"), brandingRootUrl);
    engine.loadFromModule("NeeblesInstaller", "Main");

    if (engine.rootObjects().isEmpty())
        return -1;

    return app.exec();
}
