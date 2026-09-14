#include "traysocketclient.h"

#include <LayerShellQt/Window>

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QGuiApplication>
#include <QJsonArray>
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
    value = value.trimmed();

    const qsizetype dot = value.indexOf(QLatin1Char('.'));
    if (dot >= 0)
        value.truncate(dot);

    const qsizetype modifier = value.indexOf(QLatin1Char('@'));
    if (modifier >= 0)
        value.truncate(modifier);

    value.replace(QLatin1Char('-'), QLatin1Char('_'));

    const QStringList parts =
        value.split(
            QLatin1Char('_'),
            Qt::SkipEmptyParts
        );

    if (parts.isEmpty())
        return QString();

    if (parts.size() == 1)
        return parts.at(0).toLower();

    return parts.at(0).toLower()
        + QLatin1Char('_')
        + parts.at(1).toUpper();
}

static QString bossConfigPath()
{
    const QString override = qEnvironmentVariable("NEEBLES_CONFIG").trimmed();
    if (!override.isEmpty())
        return override;

    return QDir(
        QStandardPaths::writableLocation(
            QStandardPaths::ConfigLocation
        )
    ).filePath(
        QStringLiteral("neebles/boss.json")
    );
}

static QStringList clientRoots()
{
    return {
        qEnvironmentVariable("NEEBLES_CLIENT_ROOT"),
        QDir(
            QCoreApplication::applicationDirPath()
        ).filePath(QStringLiteral("..")),
        QStringLiteral("/opt/neebles/client")
    };
}

static QJsonObject loadLanguageManifest()
{
    for (const QString &root : clientRoots()) {
        if (root.trimmed().isEmpty())
            continue;

        QFile file(
            QDir(root).filePath(
                QStringLiteral("languages/manifest.json")
            )
        );

        if (!file.open(QIODevice::ReadOnly))
            continue;

        const QJsonDocument document = QJsonDocument::fromJson(file.readAll());
        if (document.isObject())
            return document.object();
    }

    return {};
}

static QString canonicalLanguage(const QJsonObject &manifest, const QString &requested)
{
    const QString normalized = normalizeLocale(requested);
    if (normalized.isEmpty())
        return QString();

    const QJsonArray languages = manifest.value(QStringLiteral("languages")).toArray();
    for (const QJsonValue &value : languages) {
        const QString code = value.toObject().value(QStringLiteral("code")).toString();
        if (normalizeLocale(code) == normalized)
            return code;
    }

    return QString();
}

static QString manifestDefaultLanguage(const QJsonObject &manifest)
{
    const QString fallback = manifest.value(QStringLiteral("default")).toString();
    return canonicalLanguage(manifest, fallback);
}

static QString activeBossLanguage(const QJsonObject &manifest)
{
    const QString override = qEnvironmentVariable("NEEBLES_LANGUAGE");
    if (!override.trimmed().isEmpty()) {
        const QString code = canonicalLanguage(manifest, override);
        if (!code.isEmpty())
            return code;
    }

    QFile file(bossConfigPath());

    if (file.open(QIODevice::ReadOnly)) {
        const QJsonDocument document = QJsonDocument::fromJson(file.readAll());
        const QString language =
            document.object()
                .value(QStringLiteral("language"))
                .toString();

        const QString code = canonicalLanguage(manifest, language);
        if (!code.isEmpty())
            return code;
    }

    const QString system = canonicalLanguage(manifest, QLocale::system().name());
    if (!system.isEmpty())
        return system;

    return manifestDefaultLanguage(manifest);
}

static QVariantMap loadBossStrings()
{
    const QJsonObject manifest = loadLanguageManifest();
    if (manifest.isEmpty())
        return {};

    const QString language = activeBossLanguage(manifest);
    if (language.isEmpty())
        return {};

    QString fileName;
    const QJsonArray languages = manifest.value(QStringLiteral("languages")).toArray();
    for (const QJsonValue &value : languages) {
        const QJsonObject entry = value.toObject();
        if (entry.value(QStringLiteral("code")).toString() == language) {
            fileName = entry.value(QStringLiteral("file")).toString();
            break;
        }
    }

    if (fileName.trimmed().isEmpty())
        return {};

    for (const QString &root : clientRoots()) {
        if (root.trimmed().isEmpty())
            continue;

        QFile file(
            QDir(root).filePath(
                QStringLiteral("languages/") + fileName
            )
        );

        if (!file.open(QIODevice::ReadOnly))
            continue;

        const QJsonDocument document = QJsonDocument::fromJson(file.readAll());
        if (document.isObject())
            return document.object().toVariantMap();
    }

    return {};
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
