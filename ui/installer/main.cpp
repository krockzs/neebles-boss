#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QGuiApplication>
#include <QIcon>
#include <QProcess>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QTemporaryDir>
#include <QUrl>

#include "installercontroller.h"

static QString findBrandingAsset(const QString &name,
                                 const QString &payloadRoot = QString())
{
    const QString clientRoot =
        qEnvironmentVariable("NEEBLES_CLIENT_ROOT");

    const QStringList candidates = {
        payloadRoot.isEmpty()
            ? QString()
            : QDir(payloadRoot).filePath(
                  QStringLiteral("assets/branding/") + name
              ),

        clientRoot.isEmpty()
            ? QString()
            : QDir(clientRoot).filePath(
                  QStringLiteral("assets/branding/") + name
              ),

        QDir::current().filePath(
            QStringLiteral("client/assets/branding/") + name
        ),

        QDir(QCoreApplication::applicationDirPath()).filePath(
            QStringLiteral("../../client/assets/branding/") + name
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

static QString extractClientData(const QStringList &arguments,
                                 QTemporaryDir &temporary)
{
    if (arguments.size() < 6)
        return {};

    const QString archive = arguments.at(5);

    if (!QFileInfo::exists(archive))
        return {};

    if (!temporary.isValid())
        return {};

    QProcess tar;

    tar.start(
        QStringLiteral("tar"),
        {
            QStringLiteral("-xzf"),
            archive,
            QStringLiteral("-C"),
            temporary.path()
        }
    );

    if (!tar.waitForStarted(5000))
        return {};

    if (!tar.waitForFinished(30000))
        return {};

    if (
        tar.exitStatus() != QProcess::NormalExit
        || tar.exitCode() != 0
    )
        return {};

    const QString branding =
        QDir(temporary.path()).filePath(
            QStringLiteral("assets/branding")
        );

    if (!QFileInfo(branding).isDir())
        return {};

    return temporary.path();
}

int main(int argc, char *argv[])
{
    QGuiApplication app(argc, argv);

    app.setApplicationName(
        QStringLiteral("N.E.E.B.L.E.S. Installer")
    );

    app.setApplicationVersion(
        QStringLiteral("1.0.4")
    );

    app.setOrganizationName(
        QStringLiteral("N.E.E.B.L.E.S.")
    );

    QTemporaryDir payloadDirectory;

    const QString payloadRoot =
        extractClientData(
            app.arguments(),
            payloadDirectory
        );

    const QString iconPath =
        findBrandingAsset(
            QStringLiteral(
                "neebles-boss-launcher-icon.png"
            ),
            payloadRoot
        );

    if (!iconPath.isEmpty())
        app.setWindowIcon(QIcon(iconPath));

    InstallerController installer(
        app.arguments()
    );

    QQmlApplicationEngine engine;

    engine.rootContext()->setContextProperty(
        QStringLiteral("installer"),
        &installer
    );

    const QString wallPath =
        findBrandingAsset(
            QStringLiteral(
                "neebles-boss-wall-transparent.png"
            ),
            payloadRoot
        );

    QUrl brandingRootUrl;

    if (!wallPath.isEmpty()) {
        const QDir brandingDir =
            QFileInfo(wallPath).absoluteDir();

        brandingRootUrl =
            QUrl::fromLocalFile(
                brandingDir.absolutePath()
                + QDir::separator()
            );
    }

    engine.rootContext()->setContextProperty(
        QStringLiteral("brandingRootUrl"),
        brandingRootUrl
    );

    engine.loadFromModule(
        "NeeblesInstaller",
        "Main"
    );

    if (engine.rootObjects().isEmpty())
        return -1;

    return app.exec();
}
