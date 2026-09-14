#include "traysocketclient.h"

#include <LayerShellQt/Window>

#include <QDir>
#include <QFile>
#include <QGuiApplication>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLocale>
#include <QMargins>
#include <QQuickWindow>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QSize>
#include <QStandardPaths>
#include <QTimer>

static QString normalizeLocale(QString value)
{
    value.replace(QLatin1Char('-'), QLatin1Char('_'));

    const QStringList parts =
        value.split(
            QLatin1Char('_'),
            Qt::SkipEmptyParts
        );

    if (parts.isEmpty())
        return QStringLiteral("en_US");

    if (parts.size() == 1)
        return parts.at(0).toLower();

    return parts.at(0).toLower()
        + QLatin1Char('_')
        + parts.at(1).toUpper();
}

static QString activeBossLanguage()
{
    const QString override =
        qEnvironmentVariable("NEEBLES_LANGUAGE");

    if (!override.trimmed().isEmpty())
        return normalizeLocale(override);

    const QString configPath =
        QDir(
            QStandardPaths::writableLocation(
                QStandardPaths::ConfigLocation
            )
        ).filePath(
            QStringLiteral("neebles/boss.json")
        );

    QFile file(configPath);

    if (file.open(QIODevice::ReadOnly)) {
        const QJsonDocument document =
            QJsonDocument::fromJson(
                file.readAll()
            );

        const QString language =
            document.object()
                .value(
                    QStringLiteral("language")
                )
                .toString();

        if (!language.trimmed().isEmpty())
            return normalizeLocale(language);
    }

    return normalizeLocale(
        QLocale::system().name()
    );
}

static QVariantMap loadBossStrings()
{
    QString language =
        activeBossLanguage();

    const QString clientRoot =
        qEnvironmentVariable(
            "NEEBLES_CLIENT_ROOT"
        );

    const QStringList roots = {
        clientRoot,
        QDir(
            QCoreApplication::applicationDirPath()
        ).filePath(QStringLiteral("..")),
        QStringLiteral("/opt/neebles/client")
    };

    auto load = [&roots](const QString &code)
        -> QVariantMap
    {
        for (const QString &root : roots) {
            if (root.trimmed().isEmpty())
                continue;

            const QString path =
                QDir(root).filePath(
                    QStringLiteral("languages/")
                    + code
                    + QStringLiteral(".json")
                );

            QFile file(path);

            if (!file.open(QIODevice::ReadOnly))
                continue;

            const QJsonDocument document =
                QJsonDocument::fromJson(
                    file.readAll()
                );

            if (document.isObject())
                return document.object().toVariantMap();
        }

        return {};
    };

    QVariantMap strings =
        load(language);

    if (!strings.isEmpty())
        return strings;

    strings = load(
        QStringLiteral("en_US")
    );

    return strings;
}

int main(
    int argc,
    char *argv[]
)
{
    QGuiApplication app(
        argc,
        argv
    );

    QGuiApplication::setApplicationName(
        QStringLiteral(
            "N.E.E.B.L.E.S. Tray Host"
        )
    );

    QGuiApplication::setOrganizationName(
        QStringLiteral(
            "N.E.E.B.L.E.S."
        )
    );

    TraySocketClient trayClient;

    QQmlApplicationEngine engine;

    engine.rootContext()
        ->setContextProperty(
            QStringLiteral(
                "trayClient"
            ),
            &trayClient
        );

    engine.rootContext()
        ->setContextProperty(
            QStringLiteral(
                "bossStrings"
            ),
            loadBossStrings()
        );

    QObject::connect(
        &engine,
        &QQmlApplicationEngine::objectCreationFailed,
        &app,
        [] {
            QCoreApplication::exit(-1);
        },
        Qt::QueuedConnection
    );

    engine.loadFromModule(
        QStringLiteral(
            "NEEBLES.TrayHost"
        ),
        QStringLiteral(
            "Main"
        )
    );

    const auto roots =
        engine.rootObjects();

    if (roots.isEmpty()) {
        return -1;
    }

    auto *window =
        qobject_cast<QQuickWindow *>(
            roots.first()
        );

    if (!window) {
        return -1;
    }

    auto *layerWindow =
        LayerShellQt::Window::get(
            window
        );

    if (!layerWindow) {
        return -1;
    }

    layerWindow->setScope(
        QStringLiteral(
            "neebles-tray-host"
        )
    );

    layerWindow->setLayer(
        LayerShellQt::Window::LayerOverlay
    );

    LayerShellQt::Window::Anchors anchors;

    anchors.setFlag(
        LayerShellQt::Window::AnchorBottom
    );

    anchors.setFlag(
        LayerShellQt::Window::AnchorRight
    );

    layerWindow->setAnchors(
        anchors
    );

    layerWindow->setMargins(
        QMargins(
            0,
            0,
            12,
            52
        )
    );

    layerWindow->setExclusiveZone(
        0
    );

    layerWindow->setKeyboardInteractivity(
        LayerShellQt::Window::
            KeyboardInteractivityOnDemand
    );

    auto syncDesiredSize = [
        window,
        layerWindow
    ] {
        layerWindow->setDesiredSize(
            QSize(
                window->width(),
                window->height()
            )
        );
    };

    syncDesiredSize();

    QObject::connect(
        window,
        &QQuickWindow::widthChanged,
        &app,
        syncDesiredSize
    );

    QObject::connect(
        window,
        &QQuickWindow::heightChanged,
        &app,
        syncDesiredSize
    );

    layerWindow->setCloseOnDismissed(
        false
    );

    QTimer::singleShot(
        0,
        &trayClient,
        &TraySocketClient::connectToManager
    );

    return app.exec();
}
